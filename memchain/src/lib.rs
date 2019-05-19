#![feature(maybe_uninit)]

pub mod ffi;

use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc};

use blockchain_traits::{AccountMetadata, BlockchainIntrinsics, KVStore};
use oasis_types::{Address, U256};

type State<'bc> = HashMap<Address, Cow<'bc, Account>>;

const BASE_GAS: u64 = 2100;

pub struct Blockchain<'bc> {
    blocks: Vec<Block<'bc>>, // A cleaner implementation is as an intrusive linked list.
}

impl<'bc> Blockchain<'bc> {
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

impl<'bc> KVStore for Blockchain<'bc> {
    fn contains(&self, key: &[u8]) -> bool {
        self.last_block()
            .pending_transaction()
            .and_then(|tx| tx.state.get(&tx.call_stack.last().unwrap().callee))
            .map(|acct| acct.storage.contains_key(key))
            .unwrap_or(false)
    }

    fn size(&self, key: &[u8]) -> u64 {
        self.get(key).map(|v| v.len() as u64).unwrap_or(0)
    }

    fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.last_block()
            .pending_transaction()
            .and_then(|tx| tx.state.get(&tx.call_stack.last().unwrap().callee))
            .and_then(|acct| acct.storage.get(key))
            .map(Vec::as_slice)
    }

    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.last_block_mut()
            .pending_transaction_mut()
            .and_then(|tx| tx.state.get_mut(&tx.call_stack.last().unwrap().callee))
            .and_then(|acct| acct.to_mut().storage.insert(key, value));
    }
}

impl<'b> BlockchainIntrinsics for Blockchain<'b> {
    fn fetch_input(&self) -> Vec<u8> {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.input().clone())
            .unwrap_or_default()
    }

    fn input_len(&self) -> u64 {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.input().len() as u64)
            .unwrap_or_default()
    }

    fn ret(&mut self, data: Vec<u8>) {
        self.last_block_mut()
            .pending_transaction_mut()
            .map(|tx| tx.ret_buf = data);
    }

    fn err(&mut self, data: Vec<u8>) {
        self.last_block_mut()
            .pending_transaction_mut()
            .map(|tx| tx.err_buf = data);
    }

    fn fetch_ret(&self) -> Vec<u8> {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.ret_buf.to_vec())
            .unwrap_or_default()
    }

    fn ret_len(&self) -> u64 {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.ret_buf.len() as u64)
            .unwrap_or_default()
    }

    fn fetch_err(&self) -> Vec<u8> {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.err_buf.to_vec())
            .unwrap_or_default()
    }

    fn err_len(&self) -> u64 {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.err_buf.len() as u64)
            .unwrap_or_default()
    }

    fn emit(&mut self, topics: Vec<[u8; 32]>, data: Vec<u8>) {
        self.last_block_mut()
            .pending_transaction_mut()
            .map(|tx| tx.logs.push(Log { topics, data }));
    }

    fn code_at(&self, addr: &Address) -> Option<&[u8]> {
        self.last_block()
            .pending_transaction()
            .and_then(|tx| tx.state.get(&addr))
            .map(|acct| acct.code.as_slice())
    }

    fn code_len(&self, addr: &Address) -> u64 {
        self.last_block()
            .pending_transaction()
            .and_then(|tx| tx.state.get(&addr))
            .map(|acct| acct.code.len() as u64)
            .unwrap_or_default()
    }

    fn metadata_at(&self, addr: &Address) -> Option<AccountMetadata> {
        self.last_block()
            .pending_transaction()
            .and_then(|tx| tx.state.get(&addr))
            .map(|acct| AccountMetadata {
                balance: acct.balance,
                expiry: acct.expiry,
            })
    }

    fn value(&self) -> U256 {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.call_stack.last().unwrap().value)
            .expect("No pending transaction.")
    }

    fn gas(&self) -> U256 {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.call_stack.last().unwrap().gas)
            .expect("No pending transaction.")
    }

    fn sender(&self) -> Address {
        self.last_block()
            .pending_transaction()
            .map(|tx| tx.call_stack.last().unwrap().caller)
            .expect("No pending transaction.")
    }
}

pub struct Block<'bc> {
    state: State<'bc>,
    pending_transaction: Option<PendingTransaction<'bc>>,
    completed_transactions: Vec<Receipt>,
}

struct PendingTransaction<'bc> {
    state: State<'bc>,
    logs: Vec<Log>,
    call_stack: Vec<Transaction>,
    ret_buf: Vec<u8>,
    err_buf: Vec<u8>,
    outcome: TransactionOutcome,
}

impl<'bc> PendingTransaction<'bc> {
    fn input(&self) -> Vec<u8> {
        self.call_stack.last().unwrap().input.to_vec()
    }
}

impl<'bc> Block<'bc> {
    pub fn transact(
        &mut self,
        caller: Address,
        callee: Address,
        value: U256,
        input: Vec<u8>,
        gas: U256,
    ) {
        let mut receipt = Receipt {
            caller,
            callee,
            value,
            gas_used: gas,
            ret_buf: Vec::new(),
            logs: Vec::new(),
            outcome: TransactionOutcome::Success,
        };

        macro_rules! early_return {
            ($outcome:ident) => {{
                match &mut self.pending_transaction {
                    Some(ptx) => {
                        ptx.outcome = TransactionOutcome::$outcome;
                    }
                    None => {
                        receipt.outcome = TransactionOutcome::$outcome;
                        self.completed_transactions.push(receipt);
                    }
                }
                return;
            }};
        }

        // Check callee existence here so that caller balances can be modified
        // and dropped before `&mut callee` is required.
        if !self.state.contains_key(&callee) {
            early_return!(NoCallee);
        }

        let caller_acct = match self.state.get_mut(&caller) {
            Some(acct) => acct.to_mut(),
            None => early_return!(NoCaller),
        };

        if gas < U256::from(BASE_GAS) {
            early_return!(InsufficientGas);
        }

        if caller_acct.balance < (gas + value) {
            caller_acct.balance = U256::zero();
            early_return!(InsuffientFunds)
        }
        caller_acct.balance -= gas;
        caller_acct.balance -= value;

        let callee_acct = match self.state.get_mut(&callee) {
            Some(acct) => acct.to_mut(),
            None => early_return!(NoCallee),
        };
        callee_acct.balance += value;

        let tx = Transaction {
            caller,
            callee,
            value,
            input,
            gas,
        };

        let main_fn = callee_acct.main.map(|f| f.clone());

        match &mut self.pending_transaction {
            Some(ptx) => {
                ptx.call_stack.push(tx);
            }
            None => {
                self.pending_transaction = Some(PendingTransaction {
                    state: self.state.clone(),
                    logs: Vec::new(),
                    call_stack: vec![tx],
                    ret_buf: Vec::new(),
                    err_buf: Vec::new(),
                    outcome: TransactionOutcome::Success,
                })
            }
        }

        if let Some(main) = main_fn {
            unsafe { main() }
        }

        self.pending_transaction.as_mut().unwrap().call_stack.pop();
        if self
            .pending_transaction
            .as_ref()
            .unwrap()
            .call_stack
            .is_empty()
        {
            let ptx = self.pending_transaction.take().unwrap();
            receipt.outcome = ptx.outcome;
            receipt.ret_buf = ptx.ret_buf;
            receipt.logs = ptx.logs;
            self.completed_transactions.push(receipt);
        }
    }

    fn pending_transaction(&self) -> Option<&PendingTransaction> {
        self.pending_transaction.as_ref()
    }

    fn pending_transaction_mut(&mut self) -> Option<&mut PendingTransaction<'bc>> {
        self.pending_transaction.as_mut()
    }

    /// Returns the current state that does not include any pending transaction.
    pub fn state(&self) -> &State<'bc> {
        &self.state
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
}

#[repr(u8)]
pub enum TransactionOutcome {
    Success,
    NoCaller,
    NoCallee,
    InsufficientGas,
    InsuffientFunds,
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
