use blockchain_traits::TransactionOutcome;
use mantle_types::Address;

#[derive(Clone, Debug)]
pub struct Receipt {
    pub outcome: TransactionOutcome,
    pub caller: Address,
    pub callee: Address,
    pub value: u64,
    pub gas_used: u64,
    pub events: Vec<Event>,
    pub output: Vec<u8>,
}

impl blockchain_traits::Receipt for Receipt {
    type Address = Address;

    fn caller(&self) -> &Self::Address {
        &self.caller
    }

    fn callee(&self) -> &Self::Address {
        &self.callee
    }

    fn gas_used(&self) -> u64 {
        self.gas_used
    }

    fn events(&self) -> Vec<&dyn blockchain_traits::Event<Address = Self::Address>> {
        self.events.iter().map(|e| e as _).collect()
    }

    fn output(&self) -> &[u8] {
        self.output.as_slice()
    }

    fn outcome(&self) -> TransactionOutcome {
        self.outcome
    }
}

#[derive(Clone, Debug)]
pub struct Event {
    pub emitter: Address,
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

impl blockchain_traits::Event for Event {
    type Address = Address;

    fn emitter(&self) -> &Self::Address {
        &self.emitter
    }

    fn topics(&self) -> Vec<&[u8]> {
        self.topics.iter().map(AsRef::as_ref).collect()
    }

    fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
}
