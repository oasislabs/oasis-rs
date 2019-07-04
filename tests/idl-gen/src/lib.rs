pub fn test_mantle_interface(bin_name: &str, service_name: &str) {
    let mf_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());;
    let wasm_path = mf_dir.join(format!("../target/wasm32-wasi/debug/{}.wasm", bin_name));

    let iface_bytes = walrus::Module::from_file(wasm_path)
        .expect("No wasm")
        .customs
        .remove_raw("mantle-interface")
        .expect("No custom")
        .data;

    let actual = mantle_rpc::Interface::from_slice(&iface_bytes).unwrap();

    let json_path = mf_dir.join(format!("res/{}.json", service_name));
    let expected: mantle_rpc::Interface =
        serde_json::from_slice(&std::fs::read(json_path).expect("No json")).expect("Bad json");

    assert_eq!(actual, expected);
}

#[test]
fn test_mantle_build() {
    test_mantle_interface("types", "TestService");
}

#[test]
fn test_non_default_fn() {
    test_mantle_interface("non_default_fn", "NonDefaultFnService");
}
