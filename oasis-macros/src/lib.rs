#![feature(bind_by_move_pattern_guards, proc_macro_diagnostic, type_ascription)]
#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

use syn::{spanned::Spanned, visit_mut::VisitMut};

include!("utils.rs");
include!("contract_macro.rs");
include!("contract_derive.rs");
