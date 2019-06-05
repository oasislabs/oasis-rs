#![feature(linkage, trait_alias)]

pub extern crate mantle_macros as macros;

pub mod errors;
pub mod exe;
pub mod ext;

pub use crate::{exe::*, types::*};
pub use macros::{Event, Service};

pub mod reexports {
    pub use failure;
    pub use serde;
    pub use serde_cbor;
    pub use tiny_keccak;
}

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
    fn transfer<'a>(&self, value: u64) -> Result<(), crate::errors::ExtCallError>;

    fn balance(&self) -> u64;
}

impl AddressExt for Address {
    fn transfer<'a>(&self, value: u64) -> Result<(), crate::errors::ExtCallError> {
        crate::ext::transfer(self, value)
    }

    fn balance(&self) -> u64 {
        crate::ext::balance(self)
    }
}
