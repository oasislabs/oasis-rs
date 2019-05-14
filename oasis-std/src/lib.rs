#![feature(linkage, trait_alias)]

#[macro_use]
extern crate failure;
pub extern crate oasis_macros as macros;

pub mod build;
pub mod errors;
pub mod exe;
pub mod ext;
pub mod testing;
pub mod types;

#[cfg(feature = "platform-alloc")]
include!("alloc.rs");

pub mod prelude {
    pub use crate::{errors::*, exe::*, ext as oasis, types::*};
    pub use macros::{service, Event, Service};
}

pub use build::build_service;
pub use macros::{service, Event};
