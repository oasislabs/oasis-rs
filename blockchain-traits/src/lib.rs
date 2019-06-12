pub trait Address: Eq + Copy + Default + AsRef<[u8]> + std::str::FromStr {
    fn path_repr(&self) -> String;
}

/// Interface for a Blockchain-flavored key-value store.
/// The semantics of `address = Address::default()` are context-dependent but
/// generally refer to the address of the current `callee`.
pub trait KVStore {
    type Address: Address;
    /// Returns whether the key is present in account storage.
    fn contains(&self, address: &Self::Address, key: &[u8]) -> Result<bool, KVError>;

    /// Returns the size of the data stored in the account at `addr` under the given `key`.
    fn size(&self, address: &Self::Address, key: &[u8]) -> Result<u64, KVError>;

    /// Returns the data stored in the account at `addr` under the given `key`.
    fn get(&self, address: &Self::Address, key: &[u8]) -> Result<Option<&[u8]>, KVError>;

    /// Sets the data stored in the account at `addr` under the given  `key`.
    /// Overwrites any existing data.
    fn set(&mut self, address: &Self::Address, key: Vec<u8>, value: Vec<u8>)
        -> Result<(), KVError>;
}

pub trait Blockchain: KVStore {
    /// Returns the name of this blockchain.
    fn name(&self) -> &str;

    /// Executes a RPC to `callee` with provided `input` and `gas` computational resources.
    /// `value` tokens will be transferred from the `caller` to the `callee`.
    /// The `caller` is charged `gas * gas_price` for the computation.
    /// A transaction that aborts (panics) will have its changes rolled back.
    fn transact(
        &mut self,
        caller: Self::Address,
        callee: Self::Address,
        value: u64,
        input: Vec<u8>,
        gas: u64,
        gas_price: u64,
    );

    /// Returns the input provided by the calling context.
    fn fetch_input(&self) -> Vec<u8>;
    fn input_len(&self) -> u32;

    /// Returns data to the calling context.
    fn ret(&mut self, data: Vec<u8>);

    /// Returns error data to the calling context.
    fn err(&mut self, data: Vec<u8>);

    /// Returns the `ret` data of the called transaction.
    fn fetch_ret(&self) -> Vec<u8>;
    fn ret_len(&self) -> u32;

    /// Returns the `err` data of the called transaction.
    fn fetch_err(&self) -> Vec<u8>;
    fn err_len(&self) -> u32;

    /// Requests that an event be emitted in this block.
    fn emit(&mut self, topics: Vec<[u8; 32]>, data: Vec<u8>);

    /// Returns the bytecode stored at `addr`, if it exists.
    /// `None` signifies that no account exists at `addr`.
    fn code_at(&self, addr: &Self::Address) -> Option<&[u8]>;
    fn code_len(&self, addr: &Self::Address) -> u32;

    /// Returns the metadata of the account stored at `addr`, if it exists.
    fn metadata_at(&self, addr: &Self::Address) -> Option<AccountMetadata>;

    /// Returns the value sent with the current transaction.
    /// Panics if there is no pending transaction.
    fn value(&self) -> u64;

    /// Returns the gas sent with the current transaction.
    /// Panics if there is no pending transaction.
    fn gas(&self) -> u64;

    /// Returns the address of the sender of the current transaction.
    /// Panics if there is no pending transaction.
    fn sender(&self) -> &Self::Address;

    /// Returns the address of the payer of the current transaction.
    /// Panics if there is no pending transaction.
    fn payer(&self) -> &Self::Address;
}

pub struct AccountMetadata {
    pub balance: u64,
    pub expiry: Option<std::time::Duration>,
}

#[derive(Debug, PartialEq)]
pub enum KVError {
    InvalidState,
    NoAccount,
    NoPermission,
}
