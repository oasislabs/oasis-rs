use rustc::{
    hir::{self, def_id::DefId},
    ty::TyCtxt,
};
use syntax_pos::symbol::Symbol;

pub fn is_std(crate_name: Symbol) -> bool {
    crate_name == "std"
        || crate_name == "core"
        || crate_name == "alloc"
        || crate_name.as_str().starts_with("oasis_")
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
