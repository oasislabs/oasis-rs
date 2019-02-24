#[macro_use]
extern crate fixed_hash;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate uint;

pub mod ext;
pub mod types;

#[cfg(feature = "platform-alloc")]
include!("alloc.rs");

pub mod prelude {
    pub use crate::{ext::*, types::*};
}
