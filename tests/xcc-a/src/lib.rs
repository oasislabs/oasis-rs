#![cfg(test)]

#[test]
fn test_import() {
    idl_gen::test_mantle_interface("xcc-a", "ServiceA");
}
