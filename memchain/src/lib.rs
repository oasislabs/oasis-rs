#![feature(maybe_uninit)]

pub mod ffi;

use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc};

use blockchain_traits::{AccountMetadata, Blockchain, KVStore};
use oasis_types::{Address, U256};

include!("block.rs");

const BASE_GAS: u64 = 2100;

type State<'bc> = HashMap<Address, Cow<'bc, Account>>;

pub struct Memchain<'bc> {
    blocks: Vec<Block<'bc>>, // A cleaner implementation is as an intrusive linked list.
}

impl<'bc> Memchain<'bc> {
    // This function returns `Rc<RefCell<_>>` because it keeps the inner
    // `Memchain` from being moved post-construction when wrapped in
    // these structs anyway. This allows blocks to refer to each other by
    // storing a(n unmoving) pointer to their owning `Memchain`.
    pub fn new(genesis_state: State<'bc>) -> Rc<RefCell<Self>> {
        let rc_bc = Rc::new(RefCell::new(Self { blocks: Vec::new() }));

        {
            let mut bc = rc_bc.borrow_mut();

            let genesis = bc.create_block();
            genesis.state = genesis_state;

            bc.create_block(); // Create the first user block.
        }

        rc_bc
    }

    pub fn create_block(&mut self) -> &mut Block<'bc> {
        assert!(
            self.blocks.is_empty() || self.last_block().pending_transaction.is_none(),
            "Cannot create new block while there is a pending transaction"
        );
        self.blocks.push(Block {
            state: if self.blocks.is_empty() {
                State::default()
            } else {
                self.last_block().state.clone()
            },
            pending_transaction: None,
            completed_transactions: Vec::new(),
        });
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
        let last_block = self.last_block();
        match last_block.pending_transaction {
            Some(ref ptx) => &ptx.state,
            None => &last_block.state,
        }
    }

    fn current_state_mut(&mut self) -> &mut State<'bc> {
        let last_block = self.last_block_mut();
        match last_block.pending_transaction {
            Some(ref mut ptx) => &mut ptx.state,
            None => &mut last_block.state,
        }
    }
}

impl<'bc> KVStore for Memchain<'bc> {
    fn contains(&self, addr: &Address, key: &[u8]) -> bool {
        self.last_block().contains(addr, key)
    }

    fn size(&self, addr: &Address, key: &[u8]) -> u64 {
        self.get(addr, key).map(|v| v.len() as u64).unwrap_or(0)
    }

    fn get(&self, addr: &Address, key: &[u8]) -> Option<&[u8]> {
        self.last_block().get(addr, key)
    }

    fn set(&mut self, addr: &Address, key: Vec<u8>, value: Vec<u8>) {
        self.last_block_mut().set(addr, key, value)
    }
}

impl<'bc> Blockchain for Memchain<'bc> {
    fn transact(
        &mut self,
        caller: Address,
        callee: Address,
        value: U256,
        input: Vec<u8>,
        gas: U256,
        gas_price: U256,
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

    fn value(&self) -> U256 {
        self.last_block().value()
    }

    fn gas(&self) -> U256 {
        self.last_block().gas()
    }

    fn sender(&self) -> Address {
        self.last_block().sender()
    }
}

#[derive(Clone, Default)]
pub struct Account {
    pub balance: U256,
    pub code: Vec<u8>,
    pub storage: HashMap<Vec<u8>, Vec<u8>>,
    pub expiry: Option<std::time::Duration>,

    /// Callable account entrypoint. `main` takes an pointer to a
    /// `Blockchain` trait object which can be used via FFI bindings
    /// to interact with the memchain. Returns nonzero to revert transaction.
    pub main: Option<extern "C" fn(*mut dyn Blockchain) -> u16>,
}

pub struct Transaction {
    caller: Address,
    callee: Address,
    value: U256,
    input: Vec<u8>,
    gas: U256,
}

pub struct Log {
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

pub struct Receipt {
    pub outcome: TransactionOutcome,
    pub caller: Address,
    pub callee: Address,
    pub value: U256,
    pub gas_used: U256,
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
