use oasis_types::{Address, Balance};

/// A type that can be stored in blockchain storage.
pub trait Storage = serde::Serialize + serde::de::DeserializeOwned;

pub trait Service {
    /// Builds a service struct from items in Storage.
    fn coalesce() -> Self;

    /// Stores a service struct to Storage.
    fn sunder(c: Self);
}

pub trait Event {
    /// Emits an event tagged with the (keccak) hashed function name and topics.
    fn emit(&self);
}

/// The context of the current RPC.
// `Option` values are set by the user during testing.
#[derive(Default, Copy, Clone, Debug)]
pub struct Context {
    #[doc(hidden)]
    pub sender: Option<Address>,

    #[doc(hidden)]
    pub value: Option<Balance>,

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

    /// Returns the `Address` of the currently executing service.
    /// Panics if not called from within a service RPC.
    pub fn address(&self) -> Address {
        crate::backend::address()
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RpcError<T> {
    /// There was no service at the requested address.
    InvalidCallee,

    /// The caller does not have enough balance to cover the sent value.
    InsufficientFunds,

    /// The caller did not provide enough gas to complete the transaction.
    InsufficientGas,

    InvalidInput,

    InvalidOutput(Vec<u8>),

    /// The application returned an error.
    Exec(T),
}

impl<T: serde::de::DeserializeOwned> From<crate::backend::Error> for RpcError<T> {
    fn from(err: crate::backend::Error) -> Self {
        use crate::backend::Error as BackendError;
        match err {
            BackendError::Unknown => panic!("Unknown error occured."),
            BackendError::InsufficientFunds => RpcError::InsufficientFunds,
            BackendError::InvalidInput => RpcError::InvalidInput,
            BackendError::InvalidCallee => RpcError::InvalidCallee,
            BackendError::Execution { payload, .. } => {
                RpcError::Exec(match serde_cbor::from_slice::<T>(&payload) {
                    Ok(t) => t,
                    Err(_) => return RpcError::InvalidOutput(payload),
                })
            }
        }
    }
}
