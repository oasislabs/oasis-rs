use std::io::Write as _;

pub fn build_contract() -> Result<(), failure::Error> {
    let crate_name = std::env::var("CARGO_PKG_NAME")?;
    let mut contract_path =
        std::path::PathBuf::from(std::env::var("CARGO_TARGET_DIR").unwrap_or("target".to_string()))
            .canonicalize()
            .unwrap();
    contract_path.push("contract");
    contract_path.push(format!("{}.wasm", crate_name));
    println!("cargo:rustc-env=CONTRACT_PATH={}", contract_path.display());

    let contract_dir = contract_path.parent().unwrap();

    let output = std::process::Command::new("cargo")
        .args(&["build", "--target=wasm32-unknown-unknown", "--release"])
        .arg("--target-dir")
        .arg(&contract_dir)
        .args(&["--features", "deploy"])
        .output()?;

    if !output.status.success() {
        std::io::stderr().write_all(&output.stderr)?;
        return Err(failure::format_err!("Could not build contract wasm."));
    }

    let wasm_build_status = std::process::Command::new("wasm-build")
        .arg(contract_dir)
        .arg(crate_name)
        .args(&["--target", "wasm32-unknown-unknown"])
        .status();
    match wasm_build_status {
        Err(ref err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(failure::format_err!("`wasm-build` not found. Try running `cargo install owasm-utils-cli --bin wasm-build`"));
        }
        Ok(status) if !status.success() => Err(failure::format_err!(
            "`wasm-build` exited with status {}",
            status.code().unwrap()
        )),
        _ => Ok(()),
    }
}
