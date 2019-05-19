use oasis_types::{Address, U256};

pub trait KVStore {
    /// Returns whether the key is present in storage.
    fn contains(&self, key: &[u8]) -> bool;

    /// Returns the size of the data stored at `key`.
    fn size(&self, key: &[u8]) -> u64;

    /// Returns the data stored at `key`.
    fn get(&self, key: &[u8]) -> Option<&[u8]>;

    /// Sets the data stored at `key`, overwriting any existing data.
    fn set(&mut self, key: Vec<u8>, value: Vec<u8>);
}

pub trait BlockchainIntrinsics {
    /// Executes a transaction.
    fn transact(
        &mut self,
        caller: Address,
        callee: Address,
        value: U256,
        input: Vec<u8>,
        gas: U256,
    );

    /// Returns the input provided by the calling context.
    fn fetch_input(&self) -> Vec<u8>;
    fn input_len(&self) -> u64;

    /// Returns data to the calling context.
    fn ret(&mut self, data: Vec<u8>);

    /// Returns error data to the calling context.
    fn err(&mut self, data: Vec<u8>);

    /// Returns the `ret` data of the called transaction.
    fn fetch_ret(&self) -> Vec<u8>;
    fn ret_len(&self) -> u64;

    /// Returns the `err` data of the called transaction.
    fn fetch_err(&self) -> Vec<u8>;
    fn err_len(&self) -> u64;

    /// Requests that an event be emitted in this block.
    fn emit(&mut self, topics: Vec<[u8; 32]>, data: Vec<u8>);

    /// Returns the bytecode stored at `addr`, if it exists.
    /// `None` signifies that no account exists at `addr`.
    fn code_at(&self, addr: &Address) -> Option<&[u8]>;
    fn code_len(&self, addr: &Address) -> u64;

    /// Returns the metadata of the account stored at `addr`, if it exists.
    fn metadata_at(&self, addr: &Address) -> Option<AccountMetadata>;

    /// Returns the value sent with the current transaction.
    /// Panics if there is no pending transaction.
    fn value(&self) -> U256;

    /// Returns the gas sent with the current transaction.
    /// Panics if there is no pending transaction.
    fn gas(&self) -> U256;

    /// Returns the address of the sender of the current transaction.
    /// Panics if there is no pending transaction.
    fn sender(&self) -> Address;
}

pub struct AccountMetadata {
    pub balance: U256,
    pub expiry: Option<std::time::Duration>,
}
