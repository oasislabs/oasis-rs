#![feature(
    bind_by_move_pattern_guards,
    linkage,
    non_exhaustive,
    proc_macro_hygiene,
    specialization,
    trait_alias
)]
#![cfg_attr(target_os = "wasi", feature(wasi_ext))]

#[macro_use]
pub extern crate serde;
extern crate oasis_macros;

pub mod backend;
pub mod exe;

pub mod reexports {
    pub use serde;
    pub use serde_cbor;
    pub use tiny_keccak;
}

pub use oasis_macros::{default, Event, Service};
pub use oasis_types::{Address, Balance};

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
/// invoke!(addr: Address, method: impl AsRef<str>, ctx: &Context, args: ..impl Serialize);
/// ```
///
/// ## Example
///
/// ```norun
/// invoke!(
///     some_address,
///     "method",
///     &Context::default(),
///     "an arg",
///     &["some", "more", "args"],
///     42,
/// )
/// ```
#[macro_export]
macro_rules! invoke {
    ($address:expr, $method:expr, $ctx:expr, $( $arg:expr ),* $(,)?) => {{
        use crate::serde::ser::{Serializer as _, SerializeStruct as _};
        let mut serializer = $crate::reexports::serde_cbor::Serializer::new(Vec::new());
        serializer.serialize_struct("Message", 2).and_then(|mut message| {
            message.serialize_field("method", $method)?;
            message.serialize_field("payload", &( $( &$arg ),* ))?;
            message.end()
        })
        .map_err(|_| $crate::backend::Error::InvalidInput)
        .and_then(|_| $address.call($ctx, &serializer.into_inner()))
    }}
}

pub trait AddressExt {
    fn call(&self, ctx: &Context, payload: &[u8]) -> Result<Vec<u8>, crate::backend::Error>;

    fn transfer<B: Into<Balance>>(&self, value: B) -> Result<(), crate::backend::Error>;

    fn balance(&self) -> Balance;

    fn code(&self) -> Vec<u8>;
}

impl AddressExt for Address {
    fn call(&self, ctx: &Context, payload: &[u8]) -> Result<Vec<u8>, crate::backend::Error> {
        crate::backend::transact(self, ctx.value(), payload)
    }

    fn transfer<B: Into<Balance>>(&self, value: B) -> Result<(), crate::backend::Error> {
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
            "method",
            &Context::default(),
            "an arg",
            &["some", "more", "args"],
            42,
        )
        .unwrap();
    }
}
