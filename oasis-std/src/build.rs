use std::io::Write as _;

pub fn build_contract() -> Result<(), failure::Error> {
    let output = std::process::Command::new("cargo")
        .args(&["build", "--target=wasm32-unknown-unknown", "--release"])
        .args(&["--target-dir", "target/contract"])
        .args(&["--features", "deploy"])
        .output()?;

    let crate_name = std::env::var("CARGO_PKG_NAME")?;
    let lib_name = crate_name.replace("-", "_");

    if !output.status.success() {
        std::fs::write(format!("target/contract/{}.wasm", lib_name), "")?;
        std::io::stderr().write_all(&output.stderr)?;
        return Ok(());
    }

    std::process::Command::new("wasm-build")
        .args(&["target/contract", &lib_name])
        .args(&["--target", "wasm32-unknown-unknown"])
        .args(&["--final", &crate_name])
        .output()?;

    Ok(())
}
