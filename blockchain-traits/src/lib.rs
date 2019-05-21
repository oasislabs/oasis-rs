use oasis_types::{Address, U256};

/// Interface for a Blockchain-flavored key-value store.
/// The semantics of `address = Address::default()` are context-dependent but
/// generally refer to the address of the current `callee`.
pub trait KVStore {
    /// Returns whether the key is present in account storage.
    fn contains(&self, address: &Address, key: &[u8]) -> bool;

    /// Returns the size of the data stored in the account at `addr` under the given `key`.
    fn size(&self, address: &Address, key: &[u8]) -> u64;

    /// Returns the data stored in the account at `addr` under the given `key`.
    fn get(&self, address: &Address, key: &[u8]) -> Option<&[u8]>;

    /// Sets the data stored in the account at `addr` under the given  `key`.
    /// Overwrites any existing data.
    fn set(&mut self, address: &Address, key: Vec<u8>, value: Vec<u8>);
}

pub trait Blockchain: KVStore {
    /// Executes a RPC to `callee` with provided `input` and `gas` computational resources.
    /// `value` tokens will be transferred from the `caller` to the `callee`.
    /// The `caller` is charged `gas * gas_price` for the computation.
    /// A transaction that aborts (panics) will have its changes rolled back.
    fn transact(
        &mut self,
        caller: Address,
        callee: Address,
        value: U256,
        input: Vec<u8>,
        gas: U256,
        gas_price: U256,
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
