//! Generates an IDL from Rust types.
//! This library is used by registering `BuildPlugin` as a rustc callback.

#![feature(box_syntax, rustc_private)]

extern crate rustc;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_plugin;
extern crate rustc_target;
extern crate syntax;
extern crate syntax_pos;

#[macro_use]
extern crate serde;

mod dispatcher_gen;
mod error;
mod plugin;
mod rpc;
mod utils;
mod visitor;

pub use plugin::BuildPlugin;
