#![feature(bind_by_move_pattern_guards, linkage, non_exhaustive, trait_alias)]

extern crate mantle_macros;

pub mod error;
pub mod exe;
pub mod ext;

pub mod reexports {
    pub use serde;
    pub use serde_cbor;
    pub use tiny_keccak;
}

pub use mantle_macros::{Event, Service};
pub use mantle_types::Address;

pub use crate::exe::*;

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
    fn transfer<'a>(&self, value: u64) -> Result<(), crate::error::Error>;

    fn balance(&self) -> u64;
}

impl AddressExt for Address {
    fn transfer<'a>(&self, value: u64) -> Result<(), crate::error::Error> {
        crate::ext::transfer(self, value)
    }

    fn balance(&self) -> u64 {
        crate::ext::balance(self).unwrap()
    }
}
