use oasis_types::{Address, Balance};

pub trait Service {
    /// Builds a service struct from items in Storage.
    fn coalesce() -> Self;

    /// Stores a service struct to Storage.
    fn sunder(c: Self);

    /// Returns the address of this service.
    fn address(&self) -> Address {
        crate::backend::address()
    }
}

pub trait Event: crate::abi::Serialize {
    /// Emits the ABI-encoded event with the event name and indexed fields as topics.
    /// Topics are ABI-encoded and then keccak256 hashed if longer than `TOPIC_LEN`.
    /// Currently the maximum topic length is 32.
    fn emit(&self);
}

/// The maximum length of a topic. Topics longer than this will be keccak256 hashed.
const TOPIC_LEN: usize = 32;

/// ABI-encodes a topic and hashes it if its representation is longer than `TOPIC_LEN`.
#[doc(hidden)]
pub fn encode_event_topic<T: crate::abi::Serialize>(topic: &T) -> [u8; TOPIC_LEN] {
    let repr = crate::abi_encode!(topic).unwrap();
    if repr.len() <= TOPIC_LEN {
        let mut topic = [0u8; TOPIC_LEN];
        topic[..repr.len()].copy_from_slice(&repr);
        topic
    } else {
        tiny_keccak::keccak256(&repr)
    }
}

/// The context of the current RPC.
/// To create a `Context`, use `Context::default()`.
/// The default `Context` will have its `sender` be the address of the current service
/// or, when testing, the sender set by `Context::with_sender`.
///
/// You can use `Context::with_value` to transfer native tokens along with the call.
// *Note*: `Option` values are set by the user during testing.
#[derive(Default, Copy, Clone, Debug)]
pub struct Context {
    #[doc(hidden)]
    pub sender: Option<Address>,

    #[doc(hidden)]
    pub value: Option<Balance>,

    #[doc(hidden)]
    pub gas: Option<u64>,
}

impl Context {
    /// Sets the amount of computation resources available to the callee.
    /// Has no effect when called inside of a service.
    pub fn with_gas(mut self, gas: u64) -> Self {
        self.gas = Some(gas);
        self
    }

    /// Returns the `Address` of the sender of the current RPC.
    pub fn sender(&self) -> Address {
        self.sender.unwrap_or_else(crate::backend::sender)
    }

    /// Returns the AAD of the confidential execution.
    pub fn aad(&self) -> Vec<u8> {
        crate::backend::aad()
    }

    /// Returns the value with which this `Context` was created.
    pub fn value(&self) -> Balance {
        self.value.unwrap_or_else(crate::backend::value)
    }
}

impl Context {
    /// Sets the sender of the RPC receiving this `Context` as an argument.
    /// Has no effect when called inside of a service.
    #[cfg(any(test, not(target_os = "wasi")))]
    pub fn with_sender(mut self, sender: Address) -> Self {
        self.sender = Some(sender);
        self
    }

    /// Amends a Context with the value that should be transferred to the callee.
    pub fn with_value<B: Into<Balance>>(mut self, value: B) -> Self {
        self.value = Some(value.into());
        self
    }
}
