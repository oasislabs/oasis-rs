use syntax::{
    ast::{MutTy, Ty, TyKind},
    mut_visit::{self, MutVisitor},
    ptr::P,
};
use syntax_pos::symbol::Symbol;

pub struct Deborrower;

impl MutVisitor for Deborrower {
    fn visit_ty(&mut self, ty: &mut P<Ty>) {
        if let TyKind::Rptr(_, MutTy { ty: refd_ty, .. }) = ty.node {
            match refd_ty.node {
                TyKind::Path(None, path) => {
                    if path.segments.last().unwrap() == Symbol::intern("str") {
                        // *ty =
                    }
                }
                TyKind::Slice(slice_ty) => *ty = slice_ty,
                _ => *ty = refd_ty,
            }
        }
        mut_visit::noop_visit_ty(ty, self);
    }
}
