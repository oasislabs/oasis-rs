#![cfg(test)]

use std::io::Read as _;

fn test_mantle_interface(bin_name: &str, service_name: &str) {
    let mut wasm_path = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/wasm32-wasi/debug"
    ));
    wasm_path.push(format!("{}.wasm", bin_name));

    let iface_bytes = walrus::Module::from_file(wasm_path)
        .expect("No wasm")
        .customs
        .remove_raw("mantle-interface")
        .expect("No custom")
        .data;

    let mut decoder = libflate::deflate::Decoder::new(iface_bytes.as_slice());
    let mut iface_bytes = Vec::new();
    decoder.read_to_end(&mut iface_bytes).unwrap();
    let actual: mantle_rpc::Interface = serde_json::from_slice(&iface_bytes).expect("Bad custom");

    let mut json_path = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/res"));
    json_path.push(format!("{}.json", service_name));
    let expected: mantle_rpc::Interface =
        serde_json::from_slice(&std::fs::read(json_path).expect("No json")).expect("Bad json");

    assert_eq!(actual, expected);
}

#[test]
fn test_mantle_build() {
    test_mantle_interface("types", "TestService");
}

#[test]
fn test_default_fn() {
    test_mantle_interface("default_fn", "DefaultFnService");
}