#![feature(
    linkage,
    proc_macro_hygiene,
    specialization,
    trait_alias,
    // the following are used by `collections::*`
    drain_filter,
    shrink_to,
    try_reserve,
)]
#![cfg_attr(target_os = "wasi", feature(wasi_ext))]

extern crate oasis_macros;

pub mod backend;
pub mod collections;
pub mod exe;

pub mod abi {
    pub extern crate oasis_borsh;
    pub use oasis_borsh::{BorshDeserialize as Deserialize, BorshSerialize as Serialize};

    /// Encodes arguments into the format expected by Oasis services.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// use oasis_std::{abi::*, Address, AddressExt as _, Context};
    /// let method_id = 4;
    /// let payload =
    ///     oasis_std::abi_encode!(method_id, "some data", &["some", "more", "args"], 42,).unwrap();
    /// let callee = Address::default();
    /// let output = callee.call(&Context::default(), &payload).unwrap();
    /// ```
    #[macro_export]
    macro_rules! abi_encode {
        ($( $arg:expr ),* $(,)?) => {
            $crate::abi_encode!($($arg),* => Vec::new())
        };
        ($( $arg:expr ),* $(,)? => $buf:expr) => {
            Ok($buf)
                $(
                    .and_then(|mut buf| {
                        #[allow(unused)] {
                            use $crate::abi::Serialize as _;
                            $arg.serialize(&mut buf)?;
                        }
                        Ok(buf)
                    })
                )*
                .map_err(|_: std::io::Error| $crate::RpcError::InvalidInput)
        };
    }
}

#[cfg(not(target_os = "wasi"))]
#[doc(hidden)]
pub mod reexports {
    pub extern crate oasis_client; // used by generated clients
    pub extern crate oasis_test; // links the dylib containing the `backend::ext` externs
}

pub use oasis_macros::{default, Event, Service};
pub use oasis_types::{Address, Balance, RpcError};

pub use crate::exe::*;

/// This macro is used to define the "main" service.
///
/// ## Example

/// ```norun
/// fn main() {
///    oasis_std::service!(TheMainService);
/// }
/// ```
#[macro_export]
macro_rules! service {
    ($svc:path) => {};
}

pub trait AddressExt {
    fn call(&self, ctx: &Context, payload: &[u8]) -> Result<Vec<u8>, RpcError>;

    fn transfer<B: Into<Balance>>(&self, value: B) -> Result<(), RpcError>;

    fn balance(&self) -> Balance;

    fn code(&self) -> Vec<u8>;
}

impl AddressExt for Address {
    fn call(&self, ctx: &Context, payload: &[u8]) -> Result<Vec<u8>, RpcError> {
        crate::backend::transact(self, ctx.value(), payload)
    }

    fn transfer<B: Into<Balance>>(&self, value: B) -> Result<(), RpcError> {
        crate::backend::transact(self, value.into(), &[]).map(|_| ())
    }

    fn balance(&self) -> Balance {
        crate::backend::balance(self).unwrap()
    }

    fn code(&self) -> Vec<u8> {
        crate::backend::code(self).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use abi::*;

    extern crate oasis_test;

    #[test]
    fn test_invoke() {
        type T = (Address, String, Vec<u8>);
        let things = (Address::default(), "an arg", (1..100).collect::<Vec<_>>());
        let encoded = abi_encode!(&things).unwrap();
        let decoded = T::try_from_slice(&encoded).unwrap();
        assert_eq!(decoded.0, things.0);
        assert_eq!(decoded.1, things.1);
        assert_eq!(decoded.2, things.2);
    }
}
