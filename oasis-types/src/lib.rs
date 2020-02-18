#[macro_use]
extern crate derive_more;

mod address;
mod balance;

pub use address::Address;
pub use balance::Balance;

#[derive(PartialEq, Eq, Debug)]
#[repr(u32)]
#[non_exhaustive]
#[doc(hidden)]
pub enum ExtStatusCode {
    Success,
    InsufficientFunds,
    InvalidInput,
    NoAccount,
}

impl ExtStatusCode {
    pub fn from_u32(code: u32) -> Option<Self> {
        Some(match code {
            0 => ExtStatusCode::Success,
            1 => ExtStatusCode::InsufficientFunds,
            2 => ExtStatusCode::InvalidInput,
            3 => ExtStatusCode::NoAccount,
            _ => return None,
        })
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct AccountMeta {
    pub balance: u128,
    pub expiry: Option<std::time::Duration>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Event {
    pub emitter: Address,
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(target_os = "wasi", derive(Clone))]
#[cfg_attr(
    all(target_os = "wasi", feature = "serde"),
    derive(oasis_borsh::BorshSerialize, oasis_borsh::BorshDeserialize)
)]
pub enum RpcError {
    /// There was no service at the requested address.
    #[error("invalid callee")]
    InvalidCallee,

    /// The caller does not have enough balance to cover the sent value.
    #[error("caller does not have enough balance to cover transaction")]
    InsufficientFunds,

    /// The caller did not provide enough gas to complete the transaction.
    #[error("not enough gas provided to transaction")]
    InsufficientGas,

    #[error("transaction received invalid input")]
    InvalidInput,

    #[error("transaction returned invalid output")]
    InvalidOutput(Vec<u8>),

    /// The application returned an error.
    #[error("an application error occurred")]
    Execution(Vec<u8>),

    /// The gateway client encountered an error.
    #[cfg(not(target_os = "wasi"))]
    #[error("gateway error. {0}")]
    GatewayError(anyhow::Error),
}

impl RpcError {
    pub fn execution(&self) -> Option<&[u8]> {
        match self {
            RpcError::Execution(output) => Some(&output),
            _ => None,
        }
    }
}
