#![feature(trait_alias)]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate fixed_hash;
pub extern crate oasis_macros as macros;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate uint;

pub mod build;
pub mod errors;
pub mod exe;
pub mod ext;
pub mod types;

#[cfg(feature = "platform-alloc")]
include!("alloc.rs");

pub mod prelude {
    pub use crate::{errors::*, exe::*, ext as oasis, types::*};
    pub use macros::{contract, Contract};
}

pub use build::build_contract;
pub use macros::contract;
