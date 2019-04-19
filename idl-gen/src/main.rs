//! Compiles an Oasis executable and generates the RPC interface definition.
//! Usage: `GEN_IDL_FOR=<crate_name> IDL_TARGET_DIR=<dir> RUSTC_WRAPPER=idl-gen cargo build`

#![feature(box_syntax, rustc_private)]

extern crate rustc;
extern crate rustc_driver;

extern crate idl_gen;

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

fn main() -> Result<(), rustc::util::common::ErrorReported> {
    rustc_driver::init_rustc_env_logger();
    rustc_driver::report_ices_to_stderr_if_any(move || {
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

        let idl_out_dir = std::env::var_os("IDL_TARGET_DIR");
        let crate_name = std::env::var("GEN_IDL_FOR");
        let do_gen = idl_out_dir.is_some()
            && arg_value(&args, "--crate-name", |name| match &crate_name {
                Ok(crate_name) => name == crate_name,
                _ => false,
            })
            .is_some();

        let mut idl8r = idl_gen::IdlGenerator::new();
        let mut default = rustc_driver::DefaultCallbacks;
        let callbacks: &mut (dyn rustc_driver::Callbacks + Send) =
            if do_gen { &mut idl8r } else { &mut default };
        rustc_driver::run_compiler(&args, callbacks, None, None)?;

        if do_gen {
            let rpc_iface = idl8r.get();
            let mut idl_path = std::path::PathBuf::from(idl_out_dir.unwrap());
            idl_path.push(format!("{}.json", rpc_iface.service_name()));
            std::fs::write(idl_path, serde_json::to_string_pretty(rpc_iface).unwrap()).unwrap()
        }
        Ok(())
    })?
}
