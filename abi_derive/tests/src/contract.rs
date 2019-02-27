#[owasm_abi_derive::contract]
trait TestContract {
    fn constructor(&mut self) {}

    #[constant]
    // this is a test method
    fn test_method(&mut self, _quantity: u64, _address: Address) -> U256 {
        U256::zero()
    }

    /// this is a second test method
    fn second_test_method(&mut self) {}

    #[event]
    fn event(&mut self, hello: String, world: Vec<u8>) {
        assert_eq!(&hello.as_bytes(), &world.as_slice());
    }
}
