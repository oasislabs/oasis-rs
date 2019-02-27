#[macro_use]
extern crate failure;
#[macro_use]
extern crate fixed_hash;
pub extern crate owasm_abi_derive as derive;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate uint;

pub mod abi;
pub mod errors;
pub mod ext;
pub mod types;

#[cfg(feature = "platform-alloc")]
include!("alloc.rs");

pub mod prelude {
    pub use crate::{errors::*, ext::*, types::*};
    pub use owasm_abi_derive::contract;
}

pub use owasm_abi_derive::contract;
