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

pub struct SyntaxReturnType {
    pub is_result: bool,
    pub ty: ReturnType,
}

pub enum ReturnType {
    Known(syntax::ast::Ty),
    Unknown, // created when `Result` is user-defined or has no generic
    None,
}

/// Extracts the T from `-> T` or `-> Result<T, _>`
pub fn unpack_syntax_ret(ty: &syntax::ast::FunctionRetTy) -> SyntaxReturnType {
    let mut ret_ty = SyntaxReturnType {
        is_result: false,
        ty: ReturnType::None,
    };
    if let syntax::ast::FunctionRetTy::Ty(ty) = ty {
        match &ty.node {
            syntax::ast::TyKind::Path(_, path) => {
                let result = path.segments.last().unwrap();
                ret_ty.is_result = result.ident.name == Symbol::intern("Result");
                if !ret_ty.is_result {
                    if !ty.node.is_unit() {
                        ret_ty.ty = ReturnType::Known(ty.clone().into_inner());
                    }
                    return ret_ty;
                }
                if result.args.is_none() {
                    ret_ty.ty = ReturnType::Unknown;
                    return ret_ty;
                }
                if let syntax::ast::GenericArgs::AngleBracketed(syntax::ast::AngleBracketedArgs {
                    args,
                    ..
                }) = result.args.deref().as_ref().unwrap()
                {
                    if let syntax::ast::GenericArg::Type(p_ty) = &args[0] {
                        ret_ty.ty = ReturnType::Known(p_ty.clone().into_inner())
                    }
                }
            }
            _ => ret_ty.ty = ReturnType::Known(ty.clone().into_inner()),
        }
    };
    ret_ty
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

pub fn mk_parse_sess() -> syntax::parse::ParseSess {
    syntax::parse::ParseSess::new(syntax::source_map::FilePathMapping::empty())
}

#[macro_export]
macro_rules! try_parse {
    ($src:expr => $parse_fn:ident) => {{
        let sess = crate::utils::mk_parse_sess();
        let mut parser = syntax::parse::new_parser_from_source_str(
            &sess,
            syntax::source_map::FileName::Custom(String::new()),
            $src.to_string(),
        );
        parser
            .$parse_fn()
            .map_err(|mut diagnostic| diagnostic.cancel() /* drop sess */)
    }};
}

#[macro_export]
macro_rules! parse {
    ($src:expr => $parse_fn:ident) => {
        crate::try_parse!($src => $parse_fn).unwrap()
    }
}
