mod idl_gen;
mod xcc;

pub fn test_oasis_interface(bin_name: &str, service_name: &str) {
    let mf_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let wasm_path = mf_dir.join(format!("../target/wasm32-wasi/release/{}.wasm", bin_name));

    let actual =
        oasis_rpc::Interface::from_wasm_bytecode(&std::fs::read(&wasm_path).unwrap()).unwrap();

    let json_path = mf_dir.join(format!("res/{}.json", service_name));
    let expected: oasis_rpc::Interface =
        serde_json::from_slice(&std::fs::read(json_path).expect("No json")).expect("Bad json");

    assert_eq!(actual, expected);
}
