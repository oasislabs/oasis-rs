use blockchain_traits::TransactionOutcome;
use oasis_types::{Address, Event};

#[derive(Clone, Debug)]
pub struct Receipt {
    pub outcome: TransactionOutcome,
    pub caller: Address,
    pub callee: Address,
    pub value: u128,
    pub gas_used: u64,
    pub events: Vec<Event>,
    pub output: Vec<u8>,
}

impl blockchain_traits::Receipt for Receipt {
    fn caller(&self) -> &Address {
        &self.caller
    }

    fn callee(&self) -> &Address {
        &self.callee
    }

    fn gas_used(&self) -> u64 {
        self.gas_used
    }

    fn events(&self) -> Vec<&Event> {
        self.events.iter().map(|e| e as _).collect()
    }

    fn output(&self) -> &[u8] {
        self.output.as_slice()
    }

    fn outcome(&self) -> TransactionOutcome {
        self.outcome
    }
}
