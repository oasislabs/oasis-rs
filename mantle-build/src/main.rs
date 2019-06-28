//! Compiles a Mantle executable and generates the RPC interface definition.
//! Usage: `RUSTC_WRAPPER=mantle-build cargo build`

#![feature(box_syntax, rustc_private)]

extern crate rustc;
extern crate rustc_driver;

use colored::*;

// This wrapper script is inspired by `clippy-driver`.
// https://github.com/rust-lang/rust-clippy/blob/master/src/driver.rs
fn arg_value<'a>(
    args: impl IntoIterator<Item = &'a String>,
    find_arg: &str,
    pred: impl Fn(&str) -> bool,
) -> Option<&'a str> {
    let mut args = args.into_iter().map(String::as_str);

    while let Some(arg) = args.next() {
        let arg: Vec<_> = arg.splitn(2, '=').collect();
        if arg.get(0) != Some(&find_arg) {
            continue;
        }

        let value = arg.get(1).cloned().or_else(|| args.next());
        if value.as_ref().map_or(false, |p| pred(p)) {
            return value;
        }
    }
    None
}

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

        let sys_root = std::process::Command::new("rustc")
            .args(&["--print", "sysroot"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim().to_owned())
            .expect("Could not determine rustc sysroot");

        args.push("--sysroot".to_string());
        args.push(sys_root);

        let crate_name = arg_value(&args, "--crate-name", |_| true);
        let is_bin = arg_value(&args, "--crate-type", |ty| ty == "bin").is_some();
        let is_testing = arg_value(&args, "--cfg", |ty| {
            ty == "feature=\"mantle-build-compiletest\""
        })
        .is_some();
        let do_gen = is_testing || (is_bin && crate_name != Some("build_script_build"));

        let mut idl8r = mantle_build::BuildPlugin::default();
        let mut default = rustc_driver::DefaultCallbacks;
        let callbacks: &mut (dyn rustc_driver::Callbacks + Send) =
            if do_gen { &mut idl8r } else { &mut default };
        rustc_driver::run_compiler(&args, callbacks, None, None)?;

        if !do_gen || is_testing {
            return Ok(());
        }

        let crate_name = crate_name.unwrap(); // `crate_name.is_some()` when `do_gen && !is_testing`

        let rpc_iface = match idl8r.try_get() {
            Some(rpc_iface) => rpc_iface,
            None => {
                eprintln!(
                    "    {} No service defined in crate: `{}`",
                    "warning:".yellow(),
                    crate_name
                );
                return Err(rustc::util::common::ErrorReported);
            }
        };

        let mut wasm_path =
            std::path::PathBuf::from(match arg_value(&args, "--out-dir", |_| true) {
                Some(out_dir) => out_dir,
                None => return Ok(()),
            });
        wasm_path.push(format!("{}.wasm", crate_name));

        if !wasm_path.is_file() {
            return Ok(());
        }

        let mut module = walrus::Module::from_file(&wasm_path).unwrap();
        let mut encoder = libflate::deflate::Encoder::new(Vec::new());
        serde_json::to_writer(&mut encoder, rpc_iface).unwrap();
        module.customs.add(walrus::RawCustomSection {
            name: "mantle-interface".to_string(),
            data: encoder.finish().into_result().unwrap(),
        });
        module.emit_wasm_file(wasm_path).unwrap();

        Ok(())
    });

    std::process::exit(match outcome {
        Ok(_) => 0,
        Err(_) => 1,
    });
}
