use rustc::ty::TyCtxt;
use rustc_hir::{self, def_id::DefId};
use rustc_span::symbol::Symbol;
use syntax::ast;

pub fn is_std(crate_name: Symbol) -> bool {
    let crate_name = crate_name.as_str();
    crate_name == "std"
        || crate_name == "core"
        || crate_name == "alloc"
        || crate_name.starts_with("oasis")
}

/// Returns the crate name and path components of a `DefId`.
pub fn def_path(tcx: TyCtxt, did: DefId) -> (Symbol, Vec<String>) {
    let def_path_comps = tcx
        .def_path(did)
        .data
        .iter()
        .map(|dpd| dpd.data.to_string())
        .collect();
    (tcx.original_crate_name(did.krate), def_path_comps)
}

/// Returns the generic type arguments of a type path.
pub fn get_type_args<'a>(path: &'a rustc_hir::Path) -> Vec<&'a rustc_hir::Ty<'a>> {
    path.segments
        .last()
        .and_then(|segments| segments.args.as_ref())
        .map(|args| {
            args.args
                .iter()
                .filter_map(|arg| match arg {
                    rustc_hir::GenericArg::Type(ty) => Some(ty),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn is_self_ref(ty: &ast::Ty) -> bool {
    match &ty.kind {
        ast::TyKind::Rptr(_, mut_ty) => mut_ty.ty.kind.is_implicit_self(),
        _ => false,
    }
}

pub fn is_context_ref(ty: &ast::Ty) -> bool {
    match &ty.kind {
        ast::TyKind::Rptr(_, mut_ty) => match &mut_ty.ty.kind {
            ast::TyKind::Path(_, path) => path_ends_with(&path, &["oasis_std", "Context"]),
            _ => false,
        },
        _ => false,
    }
}

/// Returns whether `path` ends with `suffix`.
/// e.g, `path_is_suffix(crate::oasis_std::service, ["oasis_std", "service"]) == true`
pub fn path_ends_with(path: &ast::Path, suffix: &[&'static str]) -> bool {
    for (path_seg, suffix_seg_str) in path.segments.iter().rev().zip(suffix.iter().rev()) {
        if path_seg.ident.name != Symbol::intern(suffix_seg_str) {
            return false;
        }
    }
    true
}

pub fn make_ty(kind: ast::TyKind) -> syntax::ptr::P<ast::Ty> {
    syntax::ptr::P(ast::Ty {
        id: syntax::node_id::DUMMY_NODE_ID,
        span: rustc_span::DUMMY_SP,
        kind,
    })
}
