use oasis_types::Address;

pub trait Gateway {
    type Error;

    /// Deploys a new service with the provided initcode.
    /// `initcode` is expected to be the Wasm bytecode concatenated with the the constructor stdin.
    /// Upon success, returns the address of the new service.
    fn deploy(&self, initcode: &[u8]) -> Result<Address, Self::Error>;

    /// Returns the output of calling the service at `address` with `data` as stdin.
    fn rpc(&self, address: Address, data: &[u8]) -> Result<Vec<u8>, Self::Error>;
}
