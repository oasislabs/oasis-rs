//! An in-memory blockchain with Ethereum-like semantics.
#![feature(box_syntax)]

mod block;
#[cfg(features = "ffi")]
pub mod ffi;
mod output;
mod pending_transaction;

use std::{borrow::Cow, collections::HashMap, convert::TryInto};

use blockchain_traits::Blockchain;
use mantle_types::{AccountMeta, Address};

use block::Block;

type State<'bc> = HashMap<Address, Cow<'bc, Account>>;

#[derive(Debug)]
pub struct Memchain<'bc> {
    name: String,
    blocks: Vec<Block<'bc>>,
    base_gas: u64,
}

impl<'bc> Memchain<'bc> {
    pub fn new(name: String, genesis_state: State<'bc>, base_gas: u64) -> Self {
        let mut bc = Self {
            name,
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
    type Address = Address;
    type AccountMeta = AccountMeta;

    fn name(&self) -> &str {
        &self.name
    }

    fn block(
        &self,
        height: usize,
    ) -> Option<
        &dyn blockchain_traits::Block<Address = Self::Address, AccountMeta = Self::AccountMeta>,
    > {
        self.blocks.get(height).map(|b| {
            b as &dyn blockchain_traits::Block<
                Address = Self::Address,
                AccountMeta = Self::AccountMeta,
            >
        })
    }

    fn last_block(
        &self,
    ) -> &dyn blockchain_traits::Block<Address = Self::Address, AccountMeta = Self::AccountMeta>
    {
        self.blocks.last().unwrap()
    }

    fn last_block_mut(
        &mut self,
    ) -> &mut dyn blockchain_traits::Block<Address = Self::Address, AccountMeta = Self::AccountMeta>
    {
        self.blocks.last_mut().unwrap()
    }
}

#[derive(Clone, Default, Debug)]
pub struct Account {
    pub balance: u64,
    pub code: Vec<u8>,
    pub storage: HashMap<Vec<u8>, Vec<u8>>,
    pub expiry: Option<std::time::Duration>,

    /// Callable account entrypoint. `main` takes an pointer to a
    /// `Blockchain` trait object which can be used via FFI bindings
    /// to interact with the memchain. Returns nonzero to revert transaction.
    /// This pointer is not valid after the call to `main` has returned.
    pub main: Option<
        extern "C" fn(
            *const *mut dyn blockchain_traits::PendingTransaction<
                Address = Address,
                AccountMeta = AccountMeta,
            >,
        ) -> u16,
    >,
}

impl blockchain_traits::KVStore for Account {
    fn contains(&self, key: &[u8]) -> bool {
        self.storage.contains_key(key)
    }

    fn size(&self, key: &[u8]) -> Option<u64> {
        self.storage.get(key).map(|v| v.len() as u64)
    }

    fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.storage.get(key).map(Vec::as_slice)
    }
}

impl blockchain_traits::KVStoreMut for Account {
    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.storage.insert(key.to_vec(), value.to_vec());
    }
}

#[cfg(test)]
mod tests;
