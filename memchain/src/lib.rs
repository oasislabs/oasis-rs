#![feature(maybe_uninit)]

pub mod ffi;

use std::{
    borrow::{Borrow, Cow},
    cell::RefCell,
    collections::HashMap,
    ptr::NonNull,
    rc::Rc,
};

use blockchain_traits::{AccountMetadata, BlockchainIntrinsics, KVStore};
use oasis_types::{Address, U256};

type State<'bc> = HashMap<Address, Cow<'bc, Account>>;

pub struct Blockchain<'bc> {
    blocks: Vec<Block<'bc>>, // A cleaner implementation is as an intrusive linked list.
}

impl<'bc> Blockchain<'bc> {
    pub fn new(genesis_state: State<'bc>) -> Rc<RefCell<Self>> {
        let rc_bc = Rc::new(RefCell::new(Self { blocks: Vec::new() }));

        {
            let mut bc = rc_bc.borrow_mut();

            let genesis = bc.create_block();
            genesis.transactions.push(Transaction {
                state: genesis_state,
                call_stack: vec![Frame::default()],
                logs: Vec::new(),
            });

            bc.create_block(); // Create the first user block.
        }

        rc_bc
    }

    pub fn create_block(&mut self) -> &mut Block<'bc> {
        self.blocks.push(Block {
            number: self.blocks.len(),
            bc: NonNull::from(&*self),
            transactions: Vec::new(),
        });
        self.last_block_mut()
    }

    pub fn last_block(&self) -> &Block<'bc> {
        self.blocks.last().unwrap() // There is always at least one block
    }

    pub fn last_block_mut(&mut self) -> &mut Block<'bc> {
        self.blocks.last_mut().unwrap() // There is always at least one block.
    }

    pub fn current_state(&self) -> &State<'bc> {
        self.last_block().state()
    }

    pub fn current_state_mut(&mut self) -> &mut State<'bc> {
        self.last_block_mut().state_mut()
    }

    pub fn current_tx(&'bc self) -> Option<&'bc Transaction<'bc>> {
        self.last_block().current_tx()
    }

    pub fn with_current_tx<T, F: FnOnce(&mut Transaction<'bc>) -> T>(&mut self, f: F) -> Option<T> {
        self.last_block_mut().with_current_tx(f)
    }

    fn block(&self, number: usize) -> Option<&Block<'bc>> {
        self.blocks.get(number)
    }
}

impl<'bc> KVStore for Blockchain<'bc> {
    fn contains(&self, key: &[u8]) -> bool {
        self.current_tx()
            .and_then(Transaction::current_account)
            .map(|acct| acct.storage.contains_key(key))
            .unwrap_or(false)
    }

    fn size(&self, key: &[u8]) -> u64 {
        self.get(key).map(|v| v.len() as u64).unwrap_or(0)
    }

    fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.current_tx()
            .and_then(Transaction::current_account)
            .and_then(|acct| acct.storage.get(key))
            .map(Vec::as_slice)
    }

    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.with_current_tx(move |tx| {
            if let Some(acct) = tx.current_account_mut() {
                acct.storage.insert(key, value);
            }
        });
    }
}

impl<'b> BlockchainIntrinsics for Blockchain<'b> {
    fn input(&self) -> Vec<u8> {
        self.current_tx()
            .map(|tx| tx.current_frame().input.to_vec())
            .unwrap_or_default()
    }

    fn input_len(&self) -> u64 {
        self.current_tx()
            .map(|tx| tx.current_frame().input.len() as u64)
            .unwrap_or_default()
    }

    fn ret(&mut self, mut data: Vec<u8>) {
        self.with_current_tx(|tx| tx.current_frame_mut().ret_buf.append(&mut data));
    }

    fn ret_err(&mut self, mut data: Vec<u8>) {
        self.with_current_tx(|tx| tx.current_frame_mut().err_buf.append(&mut data));
    }

    fn emit(&mut self, topics: Vec<[u8; 32]>, data: Vec<u8>) {
        self.with_current_tx(|tx| tx.log(topics, data));
    }

    fn code_at(&self, addr: &Address) -> Option<&[u8]> {
        self.current_state()
            .get(addr)
            .map(|acct| acct.code.as_slice())
    }

    fn code_len(&self, addr: &Address) -> u64 {
        self.current_state()
            .get(addr)
            .map(|acct| acct.code.len() as u64)
            .unwrap_or_default()
    }

    fn metadata_at(&self, addr: &Address) -> Option<AccountMetadata> {
        self.current_state().get(addr).map(|acct| AccountMetadata {
            balance: acct.balance,
            expiry: acct.expiry,
        })
    }
}

pub struct Block<'bc> {
    bc: NonNull<Blockchain<'bc>>,

    // store the number instead of a pointer to the previous block because the `Blockcahin`'s
    // Vec will probably move if it reallocates.
    number: usize,
    transactions: Vec<Transaction<'bc>>,
}

impl<'bc> Block<'bc> {
    pub fn transact(
        &mut self,
        caller: Address,
        callee: Address,
        gas: U256,
        input: Vec<u8>,
    ) -> &[u8] {
        let init_frame = Frame {
            caller,
            callee,
            gas,
            input,
            ret_buf: Vec::new(),
            err_buf: Vec::new(),
        };
        self.transactions
            .push(Transaction::new(init_frame, self.state()));
        self.transactions.last().unwrap().call_stack[0]
            .ret_buf
            .as_slice()
    }

    pub fn current_tx(&self) -> Option<&Transaction<'bc>> {
        self.transactions.last()
    }

    pub fn with_current_tx<T, F: FnOnce(&mut Transaction<'bc>) -> T>(&mut self, f: F) -> Option<T> {
        self.transactions.last_mut().map(|tx| f(tx))
    }

    fn prev(&self) -> Option<&Block<'bc>> {
        unsafe { self.bc.as_ref() }.block(self.number - 1)
    }

    pub fn state(&self) -> &State<'bc> {
        match self.transactions.last() {
            Some(tx) => &tx.state,
            None => self
                .prev()
                .unwrap() // Recursion will reach genesis transaction.
                .state(),
        }
    }

    pub fn state_mut(&mut self) -> &mut State<'bc> {
        if self.transactions.is_empty() {
            self.transactions.push(Transaction {
                state: self.state().clone(),
                call_stack: Vec::new(),
                logs: Vec::new(),
            });
        }
        &mut self.transactions.last_mut().unwrap().state
    }
}

#[derive(Clone, Default, Debug)]
pub struct Account {
    pub balance: U256,
    pub code: Vec<u8>,
    pub storage: HashMap<Vec<u8>, Vec<u8>>,
    pub expiry: Option<std::time::Duration>,
    pub main: Option<unsafe extern "C" fn()>,
}

pub struct Transaction<'bc> {
    state: State<'bc>,
    call_stack: Vec<Frame>,
    logs: Vec<Log>,
}

impl<'bc> Transaction<'bc> {
    pub fn new(init_frame: Frame, init_state: &State<'bc>) -> Self {
        let new_state = init_state.clone();
        Self {
            call_stack: vec![init_frame],
            state: new_state,
            logs: Vec::new(),
        }
    }

    pub fn state(&mut self) -> &State<'bc> {
        &self.state
    }

    pub fn current_account(&self) -> Option<&Account> {
        self.state
            .get(&self.current_frame().callee)
            .map(Cow::borrow)
    }

    pub fn current_account_mut(&mut self) -> Option<&mut Account> {
        let callee = self.current_frame_mut().callee;
        self.state.get_mut(&callee).map(Cow::to_mut)
    }

    pub fn current_frame(&self) -> &Frame {
        self.call_stack.last().unwrap()
    }

    pub fn current_frame_mut(&mut self) -> &mut Frame {
        self.call_stack.last_mut().unwrap()
    }

    pub fn log(&mut self, topics: Vec<[u8; 32]>, data: Vec<u8>) {
        self.logs.push(Log { topics, data });
    }
}

#[derive(Default)]
pub struct Frame {
    pub caller: Address,
    pub callee: Address,
    pub input: Vec<u8>,
    pub gas: U256,
    pub ret_buf: Vec<u8>,
    pub err_buf: Vec<u8>,
}

pub struct Log {
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests;
