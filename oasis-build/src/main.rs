//! Compiles a Oasis executable and generates the RPC interface definition.
//! Usage: `RUSTC_WRAPPER=oasis-build cargo build`

#![feature(box_syntax, rustc_private)]

extern crate rustc;
extern crate rustc_driver;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use colored::*;
use oasis_build::{BuildTarget, DefaultCallbacks};
use oasis_rpc::import::ImportLocation;
use rustc::util::common::ErrorReported;

fn main() {
    rustc_driver::init_rustc_env_logger();
    let outcome = rustc_driver::catch_fatal_errors(move || {
        let mut args: Vec<String> = std::env::args().collect();
        if args.len() <= 1 {
            return Err(ErrorReported);
        }

        if std::path::Path::new(&args[1]).file_stem() == Some("rustc".as_ref()) {
            args.remove(1); // `RUSTC_WRAPPER` is passed `rustc` as the first arg
        }

        args.push("--sysroot".to_string());
        args.push(get_sysroot());

        let crate_name = get_arg("--crate-name", &args).cloned();
        let is_bin = get_arg("--crate-type", &args)
            .map(|t| t == "bin")
            .unwrap_or(false);
        let is_nonprimary_bin = crate_name
            .as_ref()
            .map(|n| n.starts_with("build_script") || n.starts_with('_'))
            .unwrap_or_default();
        let is_service = is_bin && !is_nonprimary_bin;
        let is_test = args.iter().any(|arg| arg == "--test");
        let is_wasi = get_arg("--target", &args).map(String::as_str) == Some("wasm32-wasi");
        let is_compiletest = args
            .iter()
            .any(|arg| arg == "feature=\"oasis-build-compiletest\"");

        let out_dir = get_arg("--out-dir", &args).map(|p| {
            let mut path = PathBuf::from(p);
            path.push(""); // ensure trailing /
            path
        });

        let mut import_semvers = Vec::new(); // for recording dependencies in the IDL
        if is_service || is_test {
            let crate_name = crate_name.as_ref().unwrap();
            let out_dir = out_dir.as_ref().unwrap();
            let gen_dir = out_dir.parent().unwrap().join("build/oasis_imports");
            std::fs::create_dir_all(&gen_dir).unwrap();

            let report_err = |err| {
                eprintln!("{}: {}\n", "error".red(), err);
                ErrorReported
            };

            let mut manifest_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
            manifest_path.push("Cargo.toml");
            let oasis_deps = OasisDependencies::load(&manifest_path).map_err(report_err)?;

            let get_oasis_deps = |crate_name: &str| -> Vec<(String, ImportLocation)> {
                oasis_deps
                    .service_configs
                    .get(crate_name)
                    .map(|cfg| cfg.dependencies.clone().into_iter().collect())
                    .unwrap_or_default()
            };

            let mut dep_stack: Vec<(String, ImportLocation)> = if is_test {
                oasis_deps.dev_dependencies.clone().into_iter().collect()
            } else {
                get_oasis_deps(&crate_name)
            };

            let mut preorder_deps = Vec::new();
            while let Some(dep) = dep_stack.pop() {
                let (dep_name, _dep_loc) = &dep;
                if preorder_deps.iter().any(|(name, _)| name == dep_name) {
                    continue;
                }

                dep_stack.append(
                    &mut oasis_deps
                        .service_configs
                        .get(dep_name)
                        .map(|cfg| cfg.dependencies.clone().into_iter().collect())
                        .unwrap_or_default(),
                );

                preorder_deps.push(dep);
            }

            let rustc_args = collect_import_rustc_args(&args);
            let mut externs = Vec::with_capacity(preorder_deps.len() * 2);
            for dep in preorder_deps.into_iter().rev() {
                let import = oasis_build::imports::build(
                    dep,
                    &gen_dir,
                    &out_dir,
                    rustc_args.iter().chain(externs.iter()).cloned().collect(),
                )
                .map_err(report_err)?;

                externs.push("--extern".to_string());
                externs.push(format!("{}={}", import.name, import.lib_path.display()));

                import_semvers.push((import.name, import.version));
            }

            args.append(&mut externs);
        }

        let build_target = if is_test {
            BuildTarget::Test
        } else if is_wasi || is_compiletest {
            BuildTarget::Wasi
        } else {
            BuildTarget::Dep
        };

        let mut idl8r = oasis_build::BuildPlugin::new(build_target, import_semvers);
        let mut default_cbs = DefaultCallbacks;
        let callbacks: &mut (dyn rustc_driver::Callbacks + Send) = if is_service || is_compiletest {
            &mut idl8r
        } else {
            &mut default_cbs
        };
        rustc_driver::run_compiler(&args, callbacks, None, None)?;

        if !(is_service && is_wasi) {
            return Ok(());
        }

        let service_name = crate_name.unwrap();

        let rpc_iface = match idl8r.try_get() {
            Some(rpc_iface) => rpc_iface,
            None => {
                eprintln!(
                    "    {} No service defined in binary: `{}`",
                    "warning:".yellow(),
                    service_name
                );
                return Ok(());
            }
        };

        let out_dir = out_dir.as_ref().unwrap();
        let wasm_path = out_dir.join(format!("{}.wasm", service_name));
        if wasm_path.is_file() {
            pack_iface_into_wasm(&rpc_iface, &wasm_path)?;
        }

        Ok(())
    });

    std::process::exit(match outcome {
        Ok(Ok(())) => 0,
        _ => 1,
    });
}

fn get_arg<'a>(arg: &str, args: &'a [String]) -> Option<&'a String> {
    args.iter()
        .position(|a| a == arg)
        .and_then(|p| args.get(p + 1))
}

fn get_sysroot() -> String {
    std::process::Command::new("rustc")
        .args(&["--print", "sysroot"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_owned())
        .expect("Could not determine rustc sysroot")
}

/// Returns the rustc args needed to build an imported services a an rlib.
/// The invoker of rustc must supply the source file as a positional arg
/// and `--crate-name=<the_import_name>`.
fn collect_import_rustc_args(args: &[String]) -> Vec<String> {
    let mut import_args = Vec::with_capacity(args.len());
    let mut skip = true; // skip `rustc`
    for arg in args {
        if skip {
            skip = false;
            continue;
        }
        if arg == "-C" || arg == "--crate-type" || arg == "--crate-name" {
            skip = true;
        } else if arg.ends_with(".rs") || arg == "--test" {
            continue;
        } else {
            import_args.push(arg.clone());
        }
    }
    import_args.push("--crate-type".to_string());
    import_args.push("lib".to_string());
    import_args
}

#[derive(Debug, Default, serde::Deserialize)]
struct OasisDependencies {
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: Dependencies,
    #[serde(default, flatten)]
    service_configs: BTreeMap<String, ServiceConfig>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct ServiceConfig {
    #[serde(default)]
    dependencies: Dependencies,
}

impl OasisDependencies {
    fn load(manifest_path: &Path) -> anyhow::Result<Self> {
        let cargo_toml: toml::Value = toml::from_slice(&std::fs::read(manifest_path)?)?;

        match cargo_toml
            .as_table()
            .and_then(|c_t| c_t.get("package").and_then(toml::Value::as_table))
            .and_then(|p| p.get("metadata").and_then(toml::Value::as_table))
            .and_then(|m| m.get("oasis"))
        {
            Some(oasis_tab) => Ok(oasis_tab.clone().try_into()?),
            None => Ok(Self::default()),
        }
    }
}

type Dependencies = BTreeMap<String, ImportLocation>;

fn pack_iface_into_wasm(
    iface: &oasis_rpc::Interface,
    wasm_path: &Path,
) -> Result<(), ErrorReported> {
    let mut module = walrus::Module::from_file(&wasm_path).unwrap();
    module.customs.add(walrus::RawCustomSection {
        name: "oasis-interface".to_string(),
        data: iface.to_vec().map_err(|_| ErrorReported)?,
    });
    module.emit_wasm_file(wasm_path).unwrap();
    Ok(())
}
