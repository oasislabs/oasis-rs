//! An in-memory blockchain with Ethereum-like semantics.
#![feature(box_syntax)]

mod block;
mod output;
mod pending_transaction;

use std::{borrow::Cow, collections::HashMap, convert::TryInto};

use oasis_types::{Address, Blockchain};

use block::Block;

type State<'bc> = HashMap<Address, Cow<'bc, Account>>;

pub type PtxPtr = *const *mut dyn oasis_types::PendingTransaction;
pub type AccountMain = extern "C" fn(PtxPtr) -> u16;

#[derive(Debug)]
pub struct Memchain<'bc> {
    name: String,
    blocks: Vec<Block<'bc>>,
    base_gas: u64,
}

impl<'bc> Memchain<'bc> {
    pub fn new<S: AsRef<str>>(name: S, genesis_state: State<'bc>, base_gas: u64) -> Self {
        let mut bc = Self {
            name: name.as_ref().to_string(),
            blocks: Vec::new(),
            base_gas,
        };
        bc.create_block_with_state(genesis_state);
        bc
    }

    pub fn create_block(&mut self) -> &mut Block<'bc> {
        self.create_block_with_state(self.blocks.last().unwrap().state.clone())
    }

    fn create_block_with_state(&mut self, state: State<'bc>) -> &mut Block<'bc> {
        self.blocks.push(Block::new(
            self.blocks.len().try_into().unwrap(),
            state,
            self.base_gas,
        ));
        self.blocks.last_mut().unwrap()
    }
}

impl<'bc> Blockchain for Memchain<'bc> {
    fn name(&self) -> &str {
        &self.name
    }

    fn block(&self, height: usize) -> Option<&dyn oasis_types::Block> {
        self.blocks
            .get(height)
            .map(|b| b as &dyn oasis_types::Block)
    }

    fn last_block(&self) -> &dyn oasis_types::Block {
        self.blocks.last().unwrap()
    }

    fn last_block_mut(&mut self) -> &mut dyn oasis_types::Block {
        self.blocks.last_mut().unwrap()
    }
}

#[derive(Clone, Default, Debug)]
pub struct Account {
    pub balance: u128,
    pub code: Vec<u8>,
    pub storage: HashMap<Vec<u8>, Vec<u8>>,
    pub expiry: Option<std::time::Duration>,

    /// Callable account entrypoint. `main` takes an pointer to a
    /// `Blockchain` trait object which can be used via FFI bindings
    /// to interact with the memchain. Returns nonzero to revert transaction.
    /// This pointer is not valid after the call to `main` has returned.
    pub main: Option<AccountMain>,
}

impl oasis_types::KVStore for Account {
    fn contains(&self, key: &[u8]) -> bool {
        self.storage.contains_key(key)
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.storage.get(key).map(Vec::to_owned)
    }
}

impl oasis_types::KVStoreMut for Account {
    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.storage.insert(key.to_vec(), value.to_vec());
    }

    fn remove(&mut self, key: &[u8]) {
        self.storage.remove(key);
    }
}

#[cfg(test)]
mod tests;
