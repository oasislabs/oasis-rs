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
    #[error("gateway error: {0}")]
    Gateway(#[source] anyhow::Error),
}

const _IMPL_SERDE_FOR_RPC_ERROR: () = {
    impl oasis_borsh::ser::BorshSerialize for RpcError {
        fn serialize<W: std::io::Write>(
            &self,
            writer: &mut W,
        ) -> std::result::Result<(), std::io::Error> {
            match self {
                RpcError::InvalidCallee => {
                    let variant_idx = 0u8;
                    writer.write_all(&variant_idx.to_le_bytes())?;
                }
                RpcError::InsufficientFunds => {
                    let variant_idx = 1u8;
                    writer.write_all(&variant_idx.to_le_bytes())?;
                }
                RpcError::InsufficientGas => {
                    let variant_idx = 2u8;
                    writer.write_all(&variant_idx.to_le_bytes())?;
                }
                RpcError::InvalidInput => {
                    let variant_idx = 3u8;
                    writer.write_all(&variant_idx.to_le_bytes())?;
                }
                RpcError::InvalidOutput(output) => {
                    let variant_idx = 4u8;
                    writer.write_all(&variant_idx.to_le_bytes())?;
                    oasis_borsh::BorshSerialize::serialize(output, writer)?;
                }
                RpcError::Execution(output) => {
                    let variant_idx = 5u8;
                    writer.write_all(&variant_idx.to_le_bytes())?;
                    oasis_borsh::BorshSerialize::serialize(output, writer)?;
                }
                #[cfg(not(target_os = "wasi"))]
                RpcError::Gateway(e) => {
                    let variant_idx = 6u8;
                    writer.write_all(&variant_idx.to_le_bytes())?;
                    oasis_borsh::BorshSerialize::serialize(&e.to_string(), writer)?;
                }
            }
            Ok(())
        }
    }

    impl oasis_borsh::de::BorshDeserialize for RpcError {
        fn deserialize<R: std::io::Read>(
            reader: &mut R,
        ) -> std::result::Result<Self, std::io::Error> {
            let mut variant_idx = [0u8];
            reader.read_exact(&mut variant_idx)?;
            let variant_idx = variant_idx[0];
            let return_value = match variant_idx {
                0u8 => RpcError::InvalidCallee,
                1u8 => RpcError::InsufficientFunds,
                2u8 => RpcError::InsufficientGas,
                3u8 => RpcError::InvalidInput,
                4u8 => RpcError::InvalidOutput(oasis_borsh::BorshDeserialize::deserialize(reader)?),
                5u8 => RpcError::Execution(oasis_borsh::BorshDeserialize::deserialize(reader)?),
                #[cfg(not(target_os = "wasi"))]
                6u8 => {
                    let err_str: String = oasis_borsh::BorshDeserialize::deserialize(reader)?;
                    RpcError::Gateway(anyhow::anyhow!(err_str))
                }
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Unexpected variant index: {}", variant_idx),
                    ))
                }
            };
            Ok(return_value)
        }
    }
};

impl RpcError {
    pub fn execution(&self) -> Option<&[u8]> {
        match self {
            RpcError::Execution(output) => Some(&output),
            _ => None,
        }
    }
}
