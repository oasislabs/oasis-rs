//! # oasis-build
//!
//! A Rust compiler plugin that turns a RPC service definition
//! into a program that runs in a blockchain-like environment.
//! Specifically, oasis-build generates boilerplate RPC code for
//! and generates an interface definition for the service.
//!
//! This library is used by registering `BuildPlugin` as a rustc callback.

#![feature(box_patterns, box_syntax, inner_deref, rustc_private)]

extern crate rustc;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_span;
extern crate rustc_target;
extern crate syntax;

mod error;
mod gen;
mod plugin;
mod rpc;
mod utils;
mod visitor;

pub use gen::{imports, insert_oasis_bindings};
pub use plugin::{BuildContext, BuildPlugin, BuildTarget};

pub struct DefaultCallbacks;
impl rustc_driver::Callbacks for DefaultCallbacks {}
