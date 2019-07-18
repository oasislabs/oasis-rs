#[macro_use]
pub mod client;
mod common;
pub mod dispatcher;
pub mod imports;

pub use crate::format_ident;

pub use client::generate as generate_client;
pub use dispatcher::insert as insert_dispatcher;
pub use imports::build as build_imports;
