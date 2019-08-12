#[test]
fn test_oasis_build() {
    common::test_oasis_interface("types", "TestService");
}

#[test]
fn test_non_default_fn() {
    common::test_oasis_interface("non_default_fn", "NonDefaultFnService");
}
