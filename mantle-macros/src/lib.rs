#![feature(
    bind_by_move_pattern_guards,
    box_patterns,
    box_syntax,
    proc_macro_diagnostic,
    proc_macro_span,
    type_ascription
)]
#![recursion_limit = "256"]

extern crate proc_macro;
#[macro_use]
extern crate proc_quote;
#[macro_use]
extern crate syn;

use syn::{spanned::Spanned as _, visit_mut::VisitMut as _};

// per rustc: "functions tagged with `#[proc_macro]` must currently reside in the root of the crate"
include!("utils.rs");
include!("service_derive.rs");
include!("event_derive.rs");
include!("testing.rs");
