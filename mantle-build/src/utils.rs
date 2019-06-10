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
        || crate_name.starts_with("mantle")
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

pub fn mk_parse_sess() -> syntax::parse::ParseSess {
    syntax::parse::ParseSess::new(syntax::source_map::FilePathMapping::empty())
}

/// Returns whether `path` ends with `suffix`.
/// e.g, `path_is_suffix(crate::mantle::service, ["mantle", "service"]) == true`
pub fn path_ends_with(path: &syntax::ast::Path, suffix: &[&'static str]) -> bool {
    for (pseg, fpseg) in path.segments.iter().rev().zip(suffix.iter().rev()) {
        if pseg.ident.name != Symbol::intern(fpseg) {
            return false;
        }
    }
    true
}

#[macro_export]
macro_rules! parse {
    ($src:expr => $parse_fn:ident) => {{
        let sess = crate::utils::mk_parse_sess();
        let mut parser = syntax::parse::new_parser_from_source_str(
            &sess,
            syntax::source_map::FileName::Custom(String::new()),
            $src.to_string(),
        );
        parser.$parse_fn().unwrap()
    }};
}
