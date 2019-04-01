#![feature(
    bind_by_move_pattern_guards,
    box_patterns,
    proc_macro_diagnostic,
    type_ascription
)]
#![recursion_limit = "256"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

use syn::{spanned::Spanned as _, visit_mut::VisitMut as _};

include!("utils.rs");
include!("contract_attr.rs");
include!("contract_derive.rs");
