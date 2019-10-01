use std::fmt;

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
/// To create a `Context`, use `Context::default()` or `Context::delegated()`.
/// The default `Context` will have its `sender` be the address of the current service
/// or, when testing, the sender set by `Context::with_sender`. A delegated `Context`
/// sets the sender to the address of the caller; this is similar to Ethereum's DELEGATECALL.
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
    /// Creates a context with the sender set to the address of
    /// the current service (i.e. `ctx.sender()`).
    #[cfg(any(test, target_os = "wasi"))]
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

#[derive(Clone, Serialize, Deserialize, failure::Fail)]
pub enum RpcError<E: Send + Sync + 'static> {
    /// There was no service at the requested address.
    InvalidCallee,

    /// The caller does not have enough balance to cover the sent value.
    InsufficientFunds,

    /// The caller did not provide enough gas to complete the transaction.
    InsufficientGas,

    InvalidInput,

    InvalidOutput(Vec<u8>),

    /// The application returned an error.
    Exec(E),
}

impl<E> From<crate::backend::Error> for RpcError<E>
where
    E: serde::de::DeserializeOwned + Send + Sync,
{
    fn from(err: crate::backend::Error) -> Self {
        use crate::backend::Error as BackendError;
        match err {
            BackendError::Unknown => panic!("Unknown error occured."),
            BackendError::InsufficientFunds => RpcError::InsufficientFunds,
            BackendError::InvalidInput => RpcError::InvalidInput,
            BackendError::InvalidCallee => RpcError::InvalidCallee,
            BackendError::Execution { payload, .. } => {
                match serde_cbor::from_slice::<E>(&payload) {
                    Ok(e) => RpcError::Exec(e),
                    Err(_) => RpcError::InvalidOutput(payload),
                }
            }
        }
    }
}

impl<E: Send + Sync> fmt::Debug for RpcError<E> {
    default fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RpcError::InvalidCallee => write!(f, "invalid callee"),
            RpcError::InsufficientFunds => write!(f, "caller has insufficient funds"),
            RpcError::InsufficientGas => write!(f, "not enough gas to complete transaction"),
            RpcError::InvalidInput => write!(f, "invalid input provided to RPC"),
            RpcError::InvalidOutput(_) => write!(f, "invalid output returned by RPC"),
            RpcError::Exec(_) => write!(f, "execution error"),
        }
    }
}

impl<E: Send + Sync> fmt::Display for RpcError<E> {
    default fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl<E: Send + Sync + fmt::Debug> fmt::Debug for RpcError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RpcError::Exec(e) => write!(f, "execution error {:?}", e),
            _ => fmt::Debug::fmt(self, f),
        }
    }
}

impl<E: Send + Sync + fmt::Display> fmt::Display for RpcError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RpcError::Exec(e) => write!(f, "execution error: {}", e),
            _ => fmt::Display::fmt(self, f),
        }
    }
}
