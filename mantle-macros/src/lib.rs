#![feature(proc_macro_diagnostic, type_ascription)]
#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate proc_quote;
#[macro_use]
extern crate syn;

use syn::spanned::Spanned as _;

// per rustc: "functions tagged with `#[proc_macro]` must currently reside in the root of the crate"
include!("utils.rs");
include!("service_derive.rs");
include!("event_derive.rs");
