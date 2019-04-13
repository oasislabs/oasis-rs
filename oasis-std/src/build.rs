use std::io::Write as _;

pub fn build_contract() -> Result<(), failure::Error> {
    let crate_name = std::env::var("CARGO_PKG_NAME")?.replace("-", "_");
    let target_dir =
        std::path::PathBuf::from(std::env::var("CARGO_TARGET_DIR").unwrap_or("target".to_string()));

    let json_dir = out_dir(target_dir.clone(), "json");
    println!("cargo:rustc-env=ABI_DIR={}", json_dir.display());

    let mut contract_path = out_dir(target_dir.clone(), "contract");
    contract_path.push(format!("{}.wasm", crate_name));
    println!("cargo:rustc-env=CONTRACT_PATH={}", contract_path.display());

    if std::env::var_os("CARGO_FEATURE_DEPLOY")
        .or_else(|| std::env::var_os("CARGO_FEATURE_TEST"))
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return Ok(());
    }

    let contract_dir = contract_path.parent().unwrap();

    let output = std::process::Command::new(std::env::var("CARGO").unwrap())
        .args(&["build", "--target=wasm32-unknown-unknown", "--release"])
        .arg("--target-dir")
        .arg(&contract_dir)
        .args(&["--features", "deploy"])
        .output()?;

    if !output.status.success() {
        if std::env::var_os("OASIS_BUILD_VERBOSE").is_some() {
            std::io::stderr().write_all(&output.stdout)?;
            std::io::stderr().write_all(&output.stderr)?;
        }
        return Ok(()); // Probably a user build error. Let Cargo display pretty error messages.
    }

    let wasm_build_status = std::process::Command::new("wasm-build")
        .arg(&contract_dir)
        .arg(&crate_name)
        .args(&["--target", "wasm32-unknown-unknown"])
        .status();

    match wasm_build_status {
        Err(ref err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(failure::format_err!("`wasm-build` not found. Try running `cargo install owasm-utils-cli --bin wasm-build`"));
        }
        Ok(status) if !status.success() => Err(failure::format_err!(
            "`wasm-build {} {} --target wasm32-unknown-unknown` exited with status {}",
            contract_dir.display(),
            crate_name,
            status.code().unwrap()
        )),
        _ => Ok(()),
    }
}

fn out_dir(target_dir: std::path::PathBuf, name: &'static str) -> std::path::PathBuf {
    let mut dir = target_dir;
    dir.push(name);
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir).expect(&format!("Could not create dir `{}`", dir.display()));
    }
    dir.canonicalize()
        .expect(&format!("Could not canonicalize `{}`", dir.display()))
}
