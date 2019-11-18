#![feature(
    bind_by_move_pattern_guards,
    linkage,
    non_exhaustive,
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
pub mod client;
pub mod collections;
pub mod exe;

pub mod abi {
    pub extern crate borsh;
    pub use borsh::{BorshDeserialize as Deserialize, BorshSerialize as Serialize};
}

#[doc(hidden)]
pub mod reexports {
    pub extern crate tiny_keccak;
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

/// Makes a transaction to an address using Oasis RPC semantics.
///
/// ## Usage
///
/// ```norun
/// invoke!(addr: Address, method_id: u32, ctx: &Context, args: ..impl Serialize);
/// ```
/// Where `method_id` is the index of the desired function in the exported IDL.
///
/// ## Example
///
/// ```norun
/// invoke!(
///     some_address,
///     0,
///     &Context::default(),
///     "an arg",
///     &["some", "more", "args"],
///     42,
/// )
/// ```
#[macro_export]
macro_rules! invoke {
    ($address:expr, $method_id:literal, $ctx:expr, $( $arg:expr ),* $(,)?) => {{
        use $crate::abi::Serialize as _;
        let mut buf = Vec::new();
        (|| -> Result<(), std::io::Error> {
            $($arg.serialize(&mut buf)?;)*
            Ok(())
        })()
        .map_err(|_| $crate::RpcError::InvalidInput)
        .and_then(|_| $address.call($ctx, &buf))
    }};
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

    extern crate oasis_test;

    #[test]
    fn test_invoke() {
        invoke!(
            Address::default(),
            0,
            &Context::default(),
            "an arg",
            &["some", "more", "args"],
            42,
        )
        .unwrap();
    }
}
