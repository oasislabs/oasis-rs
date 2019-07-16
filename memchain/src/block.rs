use blockchain_traits::TransactionOutcome;
use oasis_types::{AccountMeta, Address};

use crate::{output::Receipt, pending_transaction::PendingTransaction, State};

#[derive(Debug)]
pub struct Block<'bc> {
    pub base_gas: u64,
    pub height: u64,
    pub state: State<'bc>,
    pub completed_transactions: Vec<Receipt>,
}

impl<'bc> Block<'bc> {
    pub fn new(height: u64, state: State<'bc>, base_gas: u64) -> Self {
        Self {
            height,
            state,
            completed_transactions: Vec::new(),
            base_gas,
        }
    }
}

impl<'bc> blockchain_traits::Block for Block<'bc> {
    type Address = Address;
    type AccountMeta = AccountMeta;

    fn height(&self) -> u64 {
        self.height
    }

    fn transact(
        &mut self,
        caller: Self::Address,
        callee: Self::Address,
        payer: Self::Address,
        value: u64,
        input: &[u8],
        gas: u64,
        gas_price: u64,
    ) -> Box<dyn blockchain_traits::Receipt<Address = Self::Address>> {
        let mut receipt = Receipt {
            caller,
            callee,
            value,
            gas_used: gas,
            output: Vec::new(),
            events: Vec::new(),
            outcome: TransactionOutcome::Success,
        };

        macro_rules! early_return {
            ($outcome:ident) => {{
                receipt.outcome = TransactionOutcome::$outcome;
                self.completed_transactions.push(receipt.clone());
                return box receipt;
            }};
        }

        if !self.state.contains_key(&callee) {
            early_return!(NoAccount);
        }

        if gas < self.base_gas {
            early_return!(InsufficientGas);
        }

        match self.state.get_mut(&payer) {
            Some(payer_acct) => {
                let payer_acct = payer_acct.to_mut();
                let gas_cost = gas * gas_price;
                if payer_acct.balance < gas_cost {
                    payer_acct.balance = 0;
                    early_return!(InsufficientFunds);
                }
                payer_acct.balance -= gas_cost;
            }
            None => early_return!(NoAccount),
        };

        let mut ptx_state = self.state.clone();

        match ptx_state.get_mut(&caller) {
            Some(caller_acct) => {
                let caller_acct = caller_acct.to_mut();
                if caller_acct.balance < value {
                    early_return!(InsufficientFunds);
                }
                caller_acct.balance -= value;
            }
            None => early_return!(NoAccount),
        };

        ptx_state.get_mut(&callee).unwrap().to_mut().balance += value;

        let mut pending_transaction = PendingTransaction {
            caller,
            callee,
            value,
            input: input.to_vec(),
            outcome: TransactionOutcome::Success,
            state: ptx_state,
            events: Vec::new(),
            output: Vec::new(),
            base_gas: self.base_gas,
            gas_left: gas - self.base_gas,
        };

        if let Some(main) = self.state.get(&callee).unwrap().main {
            let ptx: &mut dyn blockchain_traits::PendingTransaction<
                Address = Address,
                AccountMeta = AccountMeta,
            > = &mut pending_transaction;
            let errno = main(unsafe {
                // Extend the lifetime, as required by the FFI type.
                // This is only unsafe if the `main` fn stores the pointer,
                // but this is disallowed by the precondition on `main`.
                &(std::mem::transmute::<&mut _, &'static mut _>(ptx) as *mut _) as *const _
            });
            if errno != 0 {
                pending_transaction.outcome = TransactionOutcome::Aborted;
            }
        }

        receipt.outcome = pending_transaction.outcome;
        receipt.output = pending_transaction.output;
        if blockchain_traits::Receipt::reverted(&receipt) {
            receipt.events.clear();
        } else {
            self.state = pending_transaction.state;
            receipt.events.append(&mut pending_transaction.events);
        }
        self.completed_transactions.push(receipt.clone());
        box receipt
    }

    fn code_at(&self, addr: &Self::Address) -> Option<&[u8]> {
        self.state.get(addr).map(|acct| acct.code.as_ref())
    }

    fn account_meta_at(&self, addr: &Self::Address) -> Option<Self::AccountMeta> {
        self.state.get(addr).map(|acct| AccountMeta {
            balance: acct.balance,
            expiry: acct.expiry,
        })
    }

    fn state_at(&self, addr: &Self::Address) -> Option<&dyn blockchain_traits::KVStore> {
        self.state.get(addr).map(|acct| &**acct as _)
    }

    fn events(&self) -> Vec<&dyn blockchain_traits::Event<Address = Self::Address>> {
        self.completed_transactions
            .iter()
            .flat_map(|r| blockchain_traits::Receipt::events(r))
            .collect()
    }

    fn receipts(&self) -> Vec<&dyn blockchain_traits::Receipt<Address = Self::Address>> {
        self.completed_transactions.iter().map(|r| r as _).collect()
    }
}
