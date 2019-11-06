#![feature(
    bind_by_move_pattern_guards,
    linkage,
    non_exhaustive,
    proc_macro_hygiene,
    specialization,
    trait_alias
)]
#![cfg_attr(target_os = "wasi", feature(wasi_ext))]

extern crate oasis_macros;

pub mod backend;
pub mod exe;

pub mod reexports {
    pub use borsh;
    pub use borsh_derive;
    pub use borsh_derive_internal;
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
        use std::io::Write as _;
        let mut buf = Vec::new();
        buf.write_all(&($method_id as u8).to_le_bytes()).unwrap();
        Ok(())
            $(.and_then(|_| {
                $crate::reexports::borsh::BorshSerialize::serialize(&$arg, &mut buf)
            }))*
            .map_err(|_| $crate::backend::Error::InvalidInput)
            .and_then(|_| $address.call($ctx, &buf))
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
            0,
            &Context::default(),
            "an arg",
            &["some", "more", "args"],
            42,
        )
        .unwrap();
    }
}
