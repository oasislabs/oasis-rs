#![feature(linkage, trait_alias)]

pub extern crate mantle_macros as macros;

pub mod build;
pub mod errors;
pub mod exe;
pub mod ext;
pub mod testing;
pub mod types;

#[cfg(feature = "platform-alloc")]
include!("alloc.rs");

pub mod prelude {
    pub use crate::{errors::*, exe::*, types::*};
    pub use macros::{Event, Service};
}

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

pub use build::build_service;
pub use exe::Context;
pub use macros::Event;
