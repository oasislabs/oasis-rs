#![feature(non_exhaustive)]

pub trait Address:
    Eq + Copy + Default + AsRef<[u8]> + std::fmt::Display + std::str::FromStr
{
    fn path_repr(&self) -> String;
}

pub trait AccountMeta {
    fn balance(&self) -> u64;
}

pub trait Blockchain {
    /// Type representing a handle to a service.
    type Address: Address;

    /// Account metadata (e.g., balance, expiry)
    type AccountMeta: AccountMeta;

    /// Returns the name of this blockchain.
    fn name(&self) -> &str;

    /// Returns the block at a given height.
    fn block(
        &self,
        height: usize,
    ) -> Option<&dyn Block<Address = Self::Address, AccountMeta = Self::AccountMeta>>;

    /// Returns a reference to the block at the current maximum height.
    fn last_block(&self) -> &dyn Block<Address = Self::Address, AccountMeta = Self::AccountMeta>;

    /// Returns a mutable reference to the block at the current maximum height.
    fn last_block_mut(
        &mut self,
    ) -> &mut dyn Block<Address = Self::Address, AccountMeta = Self::AccountMeta>;
}

pub trait Block {
    type Address: Address;
    type AccountMeta: AccountMeta;

    /// Returns the height of this block.
    fn height(&self) -> u64;

    /// Executes a RPC to `callee` with provided `input` and `gas` computational resources.
    /// `value` tokens will be transferred from the `caller` to the `callee`.
    /// The `caller` is charged `gas * gas_price` for the computation.
    /// A transaction that aborts (panics) will have its changes rolled back.
    /// This `transact` should be called by an Externally Owned Account (EOA).
    #[allow(clippy::too_many_arguments)]
    fn transact(
        &mut self,
        caller: Self::Address,
        callee: Self::Address,
        payer: Self::Address,
        value: u64,
        input: &[u8],
        gas: u64,
        gas_price: u64,
    ) -> Box<dyn Receipt<Address = Self::Address>>;

    /// Returns the bytecode stored at `addr` or `None` if the account does not exist.
    fn code_at(&self, addr: &Self::Address) -> Option<&[u8]>;

    /// Returns the metadata of the account stored at `addr`, or
    /// `None` if the account does not exist.
    fn account_meta_at(&self, addr: &Self::Address) -> Option<Self::AccountMeta>;

    /// Returns the state of the acount at `addr`, if it exists.
    fn state_at(&self, addr: &Self::Address) -> Option<&dyn KVStore>;

    /// Returns the events emitted during the course of this block.
    fn events(&self) -> Vec<&dyn Event<Address = Self::Address>>;

    /// Returns the receipts of transactions executed in this block.
    fn receipts(&self) -> Vec<&dyn Receipt<Address = Self::Address>>;
}

/// Represents the data and functionality available to a smart contract execution.
pub trait PendingTransaction {
    type Address: Address;
    type AccountMeta: AccountMeta;

    /// Returns the address of the current contract instance.
    fn address(&self) -> &Self::Address;

    /// Returns the address of the sender of the transaction.
    fn sender(&self) -> &Self::Address;

    /// Returns the value sent to the current transaction.
    fn value(&self) -> u64;

    /// Returns the input provided by the calling context.
    fn input(&self) -> &[u8];

    /// Executes a balance-transferring RPC to `callee` with provided input and value.
    /// The new transaction will inherit the gas parameters and gas payer of the top level
    /// transaction. The current account will be set as the sender.
    fn transact(
        &mut self,
        callee: Self::Address,
        value: u64,
        input: &[u8],
    ) -> Box<dyn Receipt<Address = Self::Address>>;

    /// Returns data to the calling transaction.
    fn ret(&mut self, data: &[u8]);

    /// Returns error data to the calling context.
    fn err(&mut self, data: &[u8]);

    /// Publishes a broascast message in this block.
    fn emit(&mut self, topics: &[&[u8]], data: &[u8]);

    /// Returns the state of the current account.
    fn state(&self) -> &dyn KVStore;

    /// Returns the mutable state of the current account.
    fn state_mut(&mut self) -> &mut dyn KVStoreMut;

    /// Returns the bytecode stored at `addr` or `None` if the account does not exist.
    fn code_at(&self, addr: &Self::Address) -> Option<&[u8]>;

    /// Returns the metadata of the account stored at `addr`, or
    /// `None` if the account does not exist.
    fn account_meta_at(&self, addr: &Self::Address) -> Option<Self::AccountMeta>;
}

/// Interface for a Blockchain-flavored key-value store.
pub trait KVStore {
    /// Returns whether the key is present in account storage.
    fn contains(&self, key: &[u8]) -> bool;

    /// Returns the data stored in the account at `addr` under the given `key`.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
}

pub trait KVStoreMut: KVStore {
    /// Sets the data stored in the account under the given  `key`.
    /// Overwrites any existing data.
    fn set(&mut self, key: &[u8], value: &[u8]);

    /// Removes the data stored in the account under the given  `key`.
    fn remove(&mut self, key: &[u8]);
}

pub trait Receipt {
    type Address: Address;

    fn caller(&self) -> &Self::Address;

    fn callee(&self) -> &Self::Address;

    /// Returns the total gas used during the execution of the transaction.
    fn gas_used(&self) -> u64;

    /// Returns the events emitted during the transaction.
    fn events(&self) -> Vec<&dyn Event<Address = Self::Address>>;

    /// Returns the outcome of this transaction.
    fn outcome(&self) -> TransactionOutcome;

    /// Returns the output of the transaction.
    fn output(&self) -> &[u8];

    /// Returns whether the transaction that produced this receipt was reverted.
    fn reverted(&self) -> bool {
        match self.outcome() {
            TransactionOutcome::Success => false,
            _ => true,
        }
    }
}

pub trait Event {
    type Address: Address;

    /// The address of the contract that emitted this event.
    fn emitter(&self) -> &Self::Address;

    fn topics(&self) -> Vec<&[u8]>;

    fn data(&self) -> &[u8];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
#[repr(u16)]
pub enum TransactionOutcome {
    Success,
    InsufficientFunds,
    InsufficientGas,
    InvalidInput,
    InvalidCallee,
    Aborted, // recoverable error
    Fatal,
}

impl TransactionOutcome {
    pub fn reverted(self) -> bool {
        match self {
            TransactionOutcome::Success => false,
            _ => true,
        }
    }
}
