#![cfg(test)]

#[test]
fn test_mantle_build() {
    let idl_json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/service/TestService.json"
    ))
    .unwrap();

    let actual: serde_json::Value = serde_json::from_str(&idl_json).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/res/TestService.json"
    )))
    .unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn test_default_fn() {
    let idl_json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/service/DefaultFnService.json"
    ))
    .unwrap();

    let actual: serde_json::Value = serde_json::from_str(&idl_json).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/res/DefaultFnService.json"
    )))
    .unwrap();

    assert_eq!(actual, expected);
}
