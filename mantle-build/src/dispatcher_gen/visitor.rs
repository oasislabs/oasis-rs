use syntax::{
    ast::{MutTy, Ty, TyKind},
    mut_visit::{self, MutVisitor},
    ptr::P,
};
use syntax_pos::symbol::Symbol;
