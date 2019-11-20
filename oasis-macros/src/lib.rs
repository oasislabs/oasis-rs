#![feature(proc_macro_diagnostic, type_ascription)]
#![recursion_limit = "128"]

extern crate proc_macro;

use quote::{format_ident, quote};
use syn::{parse_macro_input, spanned::Spanned as _};

macro_rules! err {
    ($( $tok:ident ).+ : $fstr:literal$(,)? $( $arg:expr ),*) => {
        err!([error] $($tok).+ : $fstr, $($arg),*)
    };
    ([ $level:ident ] $( $tok:ident ).+ : $fstr:literal$(,)? $( $arg:expr ),*) => {
        $($tok).+.span().unwrap().$level(format!($fstr, $($arg),*)).emit();
    };
}

// per rustc: "functions tagged with `#[proc_macro]` must currently reside in the root of the crate"
include!("default_attr.rs");
include!("event_derive.rs");
include!("service_derive.rs");
