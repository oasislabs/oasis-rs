use super::test_oasis_interface;

#[test]
fn test_oasis_build() {
    test_oasis_interface("types", "TestService");
}

#[test]
fn test_non_default_fn() {
    test_oasis_interface("non_default_fn", "NonDefaultFnService");
}
