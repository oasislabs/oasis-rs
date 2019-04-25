//! Generates an IDL from Rust types.
//! This library is used by registering `IdlGenerator` as a rustc callback.

#![feature(box_syntax, rustc_private)]

extern crate rustc;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_plugin;
extern crate syntax;
extern crate syntax_pos;

#[macro_use]
extern crate serde;

mod error;
mod gen;
mod rpc;
mod utils;
mod visitor;

pub use gen::IdlGenerator;
