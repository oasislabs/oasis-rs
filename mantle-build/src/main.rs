//! Compiles a Mantle executable and generates the RPC interface definition.
//! Usage: `RUSTC_WRAPPER=mantle-build cargo build`

#![feature(box_syntax, rustc_private)]

extern crate rustc;
extern crate rustc_driver;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use colored::*;
use rustc::util::common::ErrorReported;

fn main() {
    rustc_driver::init_rustc_env_logger();
    let outcome = rustc_driver::report_ices_to_stderr_if_any(move || {
        let mut args: Vec<String> = std::env::args().collect();
        if args.len() <= 1 {
            std::process::exit(1);
        }

        if std::path::Path::new(&args[1]).file_stem() == Some("rustc".as_ref()) {
            args.remove(1); // `RUSTC_WRAPPER` is passed `rustc` as the first arg
        }

        args.push("--sysroot".to_string());
        args.push(get_sysroot());

        let is_primary = std::env::var("CARGO_PRIMARY_PACKAGE")
            .map(|p| p == "1")
            .unwrap_or(false);
        let is_testing = args
            .iter()
            .any(|arg| arg == "feature=\"mantle-build-compiletest\"");

        let mut idl8r = mantle_build::BuildPlugin::default();
        let mut default_cbs = rustc_driver::DefaultCallbacks;
        let callbacks: &mut (dyn rustc_driver::Callbacks + Send) = if is_primary || is_testing {
            &mut idl8r
        } else {
            &mut default_cbs
        };

        if is_primary {
            let mut manifest_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
            manifest_path.push("Cargo.toml");
            let deps = match load_deps(&manifest_path) {
                Ok(deps) => deps,
                Err(err) => {
                    eprintln!("    {} {}", "error:".red(), err);
                    return Err(ErrorReported);
                }
            };
        }

        rustc_driver::run_compiler(&args, callbacks, None, None)?;

        if !is_primary {
            return Ok(());
        }

        let crate_name = std::env::var("CARGO_PKG_NAME").unwrap();

        let rpc_iface = match idl8r.try_get() {
            Some(rpc_iface) => rpc_iface,
            None => {
                eprintln!(
                    "    {} No service defined in crate: `{}`",
                    "warning:".yellow(),
                    crate_name
                );
                return Err(ErrorReported);
            }
        };

        let mut wasm_path =
            PathBuf::from(&args[args.iter().position(|arg| arg == "--out-dir").unwrap() + 1]);
        wasm_path.push(format!("{}.wasm", crate_name));

        if wasm_path.is_file() {
            pack_iface_into_wasm(&rpc_iface, &wasm_path)?;
        }

        Ok(())
    });

    std::process::exit(match outcome {
        Ok(_) => 0,
        Err(_) => 1,
    });
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

pub enum DependenciesError {
    TomlParse(toml::de::Error),
}

impl std::fmt::Display for DependenciesError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use DependenciesError::*;
        match self {
            TomlParse(err) => write!(f, "Could not parse Mantle dependencies: {}", err),
        }
    }
}

fn load_deps(manifest_path: &Path) -> Result<BTreeMap<String, String>, DependenciesError> {
    let cargo_toml: toml::Value = toml::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
    Ok(cargo_toml
        .as_table()
        .and_then(|c_t| c_t.get("package").and_then(toml::Value::as_table))
        .and_then(|p| p.get("metadata").and_then(toml::Value::as_table))
        .and_then(|m| m.get("mantle-dependencies"))
        .cloned()
        .map(|d| d.try_into::<BTreeMap<String, String>>())
        .unwrap_or(Ok(BTreeMap::new()))
        .map_err(|err| DependenciesError::TomlParse(err))?)
}

fn pack_iface_into_wasm(
    iface: &mantle_rpc::Interface,
    wasm_path: &Path,
) -> Result<(), ErrorReported> {
    let mut module = walrus::Module::from_file(&wasm_path).unwrap();
    module.customs.add(walrus::RawCustomSection {
        name: "mantle-interface".to_string(),
        data: iface.to_vec().map_err(|_| ErrorReported)?,
    });
    module.emit_wasm_file(wasm_path).unwrap();
    Ok(())
}
