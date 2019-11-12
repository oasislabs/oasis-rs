use oasis_std::abi::*;
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct NonXccType;

#[cfg(test)]
#[test]
fn test_import() {
    idl_gen::test_oasis_interface("a", "ServiceA");
}
