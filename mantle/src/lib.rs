#![feature(linkage, trait_alias)]

pub extern crate mantle_macros as macros;

pub mod exe;

pub use crate::{exe::*, types::*};
pub use macros::{Event, Service};

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

pub mod reexports {
    pub use failure;
    pub use serde;
    pub use serde_cbor;
    pub use tiny_keccak;
}
