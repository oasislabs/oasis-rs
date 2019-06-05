use crate::{ext, types::Address};

/// A type that can be stored in blockchain storage.
pub trait Storage = serde::Serialize + serde::de::DeserializeOwned;

pub trait Service {
    /// Builds a service struct from items in Storage.
    fn coalesce() -> Self;

    /// Stores a service struct to Storage.
    fn sunder(c: Self);
}

pub trait Event {
    /// A struct implementing the builder pattern for setting topics.
    ///
    /// For example,
    /// ```
    /// #[derive(Event)]
    /// struct MyEvent {
    ///    #[indexed]
    ///    my_topic: u64
    ///    #[indexed]
    ///    my_other_topic: String,
    /// }
    ///
    /// let topics: Vec<Vec<u8>> = MyTopics::Topics::default()
    ///    .set_my_other_topic("hi".to_string())
    ///    .hash();
    /// // topics = vec![0, keccak256(abi_encode(my_other_topic))]
    /// ```
    type Topics;

    /// Emits an event tagged with the (keccak) hashed function name and topics.
    fn emit(&self);
}

/// The context of the current RPC.
// `Option` values are set by the user. `None` when populated by runting (during call/deploy).
#[derive(Default, Copy, Clone, Debug)]
pub struct Context {
    #[doc(hidden)]
    pub sender: Option<Address>,

    #[doc(hidden)]
    pub value: Option<u64>,

    #[doc(hidden)]
    pub gas: Option<u64>,

    #[doc(hidden)]
    pub call_type: CallType,
}

#[derive(Copy, Clone, Debug)]
pub enum CallType {
    Default,
    Delegated,
    Constant,
}

impl Default for CallType {
    fn default() -> Self {
        CallType::Default
    }
}

impl Context {
    pub fn delegated() -> Self {
        Self {
            call_type: CallType::Delegated,
            ..Default::default()
        }
    }

    /// Sets the sender of the RPC receiving this `Context` as an argument.
    /// Has no effect when called inside of a service.
    pub fn with_sender(mut self, sender: Address) -> Self {
        self.sender = Some(sender);
        self
    }

    /// Amends a Context with the value that should be transferred to the callee.
    pub fn with_value(mut self, value: u64) -> Self {
        self.value = Some(value);
        self
    }

    /// Sets the amount of computation resources available to the callee.
    /// Payed for by the `payer` of the `Context`.
    pub fn with_gas(mut self, gas: u64) -> Self {
        self.gas = Some(gas);
        self
    }

    /// Returns the `Address` of the sender of the current RPC.
    pub fn sender(&self) -> Address {
        self.sender.unwrap_or_else(ext::sender)
    }

    /// Returns the `Address` of the currently executing service.
    /// Panics if not currently in a service.
    pub fn address(&self) -> Address {
        ext::address()
    }

    /// Returns the value with which this `Context` was created.
    pub fn value(&self) -> u64 {
        self.value.unwrap_or_else(ext::value)
    }

    /// Returns the remaining gas allocated to this transaction.
    pub fn gas_left(&self) -> u64 {
        ext::gas_left()
    }
}
