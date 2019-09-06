pub fn test_oasis_interface(bin_name: &str, service_name: &str) {
    let mf_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let wasm_path = mf_dir.join(format!("../target/wasm32-wasi/debug/{}.wasm", bin_name));

    let iface_bytes = walrus::Module::from_file(wasm_path)
        .expect("No wasm")
        .customs
        .remove_raw("oasis-interface")
        .expect("No custom")
        .data;

    let actual = oasis_rpc::Interface::from_slice(&iface_bytes).unwrap();

    let json_path = mf_dir.join(format!("res/{}.json", service_name));
    let expected: oasis_rpc::Interface =
        serde_json::from_slice(&std::fs::read(json_path).expect("No json")).expect("Bad json");

    assert_eq!(actual, expected);
}

#[test]
fn test_oasis_build() {
    test_oasis_interface("types", "TestService");
}

#[test]
fn test_non_default_fn() {
    test_oasis_interface("non_default_fn", "NonDefaultFnService");
}
