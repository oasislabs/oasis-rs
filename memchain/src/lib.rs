//! An in-memory blockchain with Ethereum-like semantics.
#![feature(maybe_uninit)]

mod block;
pub mod ffi;

pub const BASE_GAS: u64 = 2100;

use std::{borrow::Cow, collections::HashMap};

use blockchain_traits::{AccountMetadata, Blockchain, KVError, KVStore};
use oasis_types::Address;

use block::Block;

type State<'bc> = HashMap<Address, Cow<'bc, Account>>;

pub struct Memchain<'bc> {
    name: String,
    blocks: Vec<Block<'bc>>, // A cleaner implementation is as an intrusive linked list.
}

impl<'bc> Memchain<'bc> {
    pub fn new(name: String, genesis_state: State<'bc>) -> Self {
        let mut bc = Self {
            name,
            blocks: Vec::new(),
        };
        bc.create_block_with_state(genesis_state);
        bc
    }

    pub fn create_block(&mut self) -> &mut Block<'bc> {
        assert!(
            !self.last_block().has_pending_transaction(),
            "Cannot create new block while there is a pending transaction"
        );
        self.create_block_with_state(self.last_block().current_state().clone())
    }

    fn create_block_with_state(&mut self, state: State<'bc>) -> &mut Block<'bc> {
        self.blocks.push(Block::new(self.name.to_string(), state));
        self.last_block_mut()
    }

    pub fn last_block(&self) -> &Block<'bc> {
        self.blocks.last().unwrap() // There is always at least one block
    }

    pub fn last_block_mut(&mut self) -> &mut Block<'bc> {
        self.blocks.last_mut().unwrap() // There is always at least one block.
    }

    pub fn block(&self, height: usize) -> Option<&Block<'bc>> {
        self.blocks.get(height)
    }

    fn current_state(&self) -> &State<'bc> {
        self.last_block().current_state()
    }
}

impl<'bc> KVStore for Memchain<'bc> {
    type Address = Address;

    fn contains(&self, addr: &Address, key: &[u8]) -> Result<bool, KVError> {
        self.last_block().contains(addr, key)
    }

    fn size(&self, addr: &Address, key: &[u8]) -> Result<u64, KVError> {
        self.last_block().size(addr, key)
    }

    fn get(&self, addr: &Address, key: &[u8]) -> Result<Option<&[u8]>, KVError> {
        self.last_block().get(addr, key)
    }

    fn set(&mut self, addr: &Address, key: Vec<u8>, value: Vec<u8>) -> Result<(), KVError> {
        self.last_block_mut().set(addr, key, value)
    }
}

impl<'bc> Blockchain for Memchain<'bc> {
    fn name(&self) -> &str {
        &self.name
    }

    fn transact(
        &mut self,
        caller: Address,
        callee: Address,
        value: u64,
        input: Vec<u8>,
        gas: u64,
        gas_price: u64,
    ) {
        self.last_block_mut()
            .transact(caller, callee, value, input, gas, gas_price);
    }

    fn fetch_input(&self) -> Vec<u8> {
        self.last_block().fetch_input()
    }

    fn input_len(&self) -> u64 {
        self.last_block().input_len()
    }

    fn ret(&mut self, data: Vec<u8>) {
        self.last_block_mut().ret(data)
    }

    fn err(&mut self, data: Vec<u8>) {
        self.last_block_mut().err(data)
    }

    fn fetch_ret(&self) -> Vec<u8> {
        self.last_block().fetch_ret()
    }

    fn ret_len(&self) -> u64 {
        self.last_block().ret_len()
    }

    fn fetch_err(&self) -> Vec<u8> {
        self.last_block().fetch_err()
    }

    fn err_len(&self) -> u64 {
        self.last_block().err_len()
    }

    fn emit(&mut self, topics: Vec<[u8; 32]>, data: Vec<u8>) {
        self.last_block_mut().emit(topics, data)
    }

    fn code_at(&self, addr: &Address) -> Option<&[u8]> {
        self.last_block().code_at(addr)
    }

    fn code_len(&self, addr: &Address) -> u64 {
        self.last_block().code_len(addr)
    }

    fn metadata_at(&self, addr: &Address) -> Option<AccountMetadata> {
        self.last_block().metadata_at(addr)
    }

    fn value(&self) -> u64 {
        self.last_block().value()
    }

    fn gas(&self) -> u64 {
        self.last_block().gas()
    }

    fn sender(&self) -> &Address {
        self.last_block().sender()
    }
}

#[derive(Clone, Default)]
pub struct Account {
    pub balance: u64,
    pub code: Vec<u8>,
    pub storage: HashMap<Vec<u8>, Vec<u8>>,
    pub expiry: Option<std::time::Duration>,

    /// Callable account entrypoint. `main` takes an pointer to a
    /// `Blockchain` trait object which can be used via FFI bindings
    /// to interact with the memchain. Returns nonzero to revert transaction.
    pub main: Option<extern "C" fn(*const *mut dyn Blockchain<Address = Address>) -> u16>,
}

pub struct Transaction {
    caller: Address,
    callee: Address,
    value: u64,
    input: Vec<u8>,
    gas: u64,
}

pub struct Log {
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

pub struct Receipt {
    pub outcome: TransactionOutcome,
    pub caller: Address,
    pub callee: Address,
    pub value: u64,
    pub gas_used: u64,
    pub logs: Vec<Log>,
    pub ret_buf: Vec<u8>,
    pub err_buf: Vec<u8>,
}

#[derive(Debug)]
#[repr(u8)]
pub enum TransactionOutcome {
    Success,
    Aborted,
    NoCaller,
    InvalidCaller,
    NoCallee,
    InsufficientGas,
    InsuffientFunds,
    InvalidOperation,
}

impl TransactionOutcome {
    pub fn reverted(&self) -> bool {
        match self {
            TransactionOutcome::Success => false,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests;
