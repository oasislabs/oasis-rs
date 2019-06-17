#![feature(bind_by_move_pattern_guards, linkage, non_exhaustive, trait_alias)]
#![cfg_attr(target_os = "wasi", feature(wasi_ext))]

extern crate mantle_macros;

pub mod backend;
pub mod error;
pub mod exe;

pub mod reexports {
    pub use serde;
    pub use serde_cbor;
    pub use tiny_keccak;
}

pub use mantle_macros::{Event, Service};
pub use mantle_types::Address;

pub use crate::{error::Error, exe::*};

/// This macro is used to define the "main" service.
///
/// ## Example

/// ```norun
/// fn main() {
///    mantle::service!(TheMainService);
/// }
/// ```
#[macro_export]
macro_rules! service {
    ($svc:path) => {};
}

pub trait AddressExt {
    fn transfer(&self, value: u64) -> Result<(), crate::error::Error>;

    fn balance(&self) -> u64;

    fn code(&self) -> Vec<u8>;
}

impl AddressExt for Address {
    fn transfer(&self, value: u64) -> Result<(), crate::error::Error> {
        crate::backend::transact(self, value, &[]).map(|_| ())
    }

    fn balance(&self) -> u64 {
        crate::backend::balance(self).unwrap()
    }

    fn code(&self) -> Vec<u8> {
        crate::backend::code(self).unwrap()
    }
}
