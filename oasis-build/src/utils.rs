use rustc::{
    hir::{self, def_id::DefId},
    ty::TyCtxt,
};
use syntax_pos::symbol::Symbol;

pub fn is_std(crate_name: Symbol) -> bool {
    let crate_name = crate_name.as_str();
    crate_name == "std"
        || crate_name == "core"
        || crate_name == "alloc"
        || crate_name == "map_vec"
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
pub fn get_type_args(path: &hir::Path) -> Vec<&hir::Ty> {
    path.segments
        .last()
        .and_then(|segments| segments.args.as_ref())
        .map(|args| {
            args.args
                .iter()
                .filter_map(|arg| match arg {
                    hir::GenericArg::Type(ty) => Some(ty),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn is_self_ref(ty: &syntax::ast::Ty) -> bool {
    match &ty.node {
        syntax::ast::TyKind::Rptr(_, mut_ty) => mut_ty.ty.node.is_implicit_self(),
        _ => false,
    }
}

pub fn is_context_ref(ty: &syntax::ast::Ty) -> bool {
    match &ty.node {
        syntax::ast::TyKind::Rptr(_, mut_ty) => match &mut_ty.ty.node {
            syntax::ast::TyKind::Path(_, path) => path_ends_with(&path, &["oasis_std", "Context"]),
            _ => false,
        },
        _ => false,
    }
}

/// Returns whether `path` ends with `suffix`.
/// e.g, `path_is_suffix(crate::oasis_std::service, ["oasis_std", "service"]) == true`
pub fn path_ends_with(path: &syntax::ast::Path, suffix: &[&'static str]) -> bool {
    for (path_seg, suffix_seg_str) in path.segments.iter().rev().zip(suffix.iter().rev()) {
        if path_seg.ident.name != Symbol::intern(suffix_seg_str) {
            return false;
        }
    }
    true
}
