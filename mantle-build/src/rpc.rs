//! Types representing an "IR" for RPC interface definitions.

use std::collections::BTreeSet;

use rustc::{
    hir::{self, def_id::DefId, Body, FnDecl},
    ty::{self, AdtDef, TyCtxt, TyS},
    util::nodemap::FxHashMap,
};
use syntax_pos::symbol::Symbol;

use mantle_rpc::{
    Field, Function, Import, IndexedField, Interface, StateConstructor, StateMutability, Type,
    TypeDef,
};

use crate::error::UnsupportedTypeError;

// faq: why return a vec of errors? so that the user can see and correct them all at once.
pub fn convert_interface(
    tcx: TyCtxt,
    name: Symbol,
    // the following use BTreeSets to ensure idl is deterministic
    imports: BTreeSet<(Symbol, String)>, // (name, version)
    adt_defs: BTreeSet<(&AdtDef, bool)>, // (adt_def, is_event)
    event_indices: &FxHashMap<Symbol, Vec<Symbol>>,
    fns: &[(Symbol, &FnDecl, &Body)],
) -> Result<Interface, Vec<UnsupportedTypeError>> {
    let mut errs = Vec::new();

    let imports = imports
        .into_iter()
        .map(|(name, version)| Import {
            name: name.to_string(),
            version,
        })
        .collect();

    let mut type_defs = Vec::with_capacity(adt_defs.len());
    for (adt_def, is_event) in adt_defs.iter() {
        match convert_type_def(tcx, adt_def, *is_event) {
            Ok(mut event_def) => {
                if let TypeDef::Event {
                    name,
                    ref mut fields,
                } = &mut event_def
                {
                    if let Some(indexed_fields) = event_indices.get(&Symbol::intern(name)) {
                        for field in fields.iter_mut() {
                            field.indexed = indexed_fields
                                .iter()
                                .any(|f| *f == Symbol::intern(field.name.as_str()));
                        }
                    }
                }
                type_defs.push(event_def);
            }
            Err(err) => errs.push(err),
        }
    }

    let mut ctor = None;
    let mut functions = Vec::with_capacity(fns.len());
    let mut has_default_function = false;
    for (name, decl, body) in fns.iter() {
        if name.as_str() == "new" {
            match convert_state_ctor(tcx, decl, body) {
                Ok(constructor) => ctor = Some(constructor),
                Err(mut errz) => errs.append(&mut errz),
            }
        } else {
            match convert_function(tcx, *name, decl, body) {
                Ok(ref rpc_fn)
                    if name.as_str() == "default"
                        && rpc_fn.inputs.is_empty()
                        && rpc_fn.output.is_none() =>
                {
                    has_default_function = true;
                }
                Ok(rpc_fn) => functions.push(rpc_fn),
                Err(mut errz) => errs.append(&mut errz),
            }
        }
    }

    if !errs.is_empty() {
        Err(errs)
    } else {
        Ok(Interface {
            name: name.to_string(),
            namespace: tcx.crate_name.to_string(),
            imports,
            type_defs,
            constructor: ctor.unwrap(),
            functions,
            has_default_function,
            mantle_build_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }
}

fn convert_state_ctor(
    tcx: TyCtxt,
    decl: &FnDecl,
    body: &Body,
) -> Result<StateConstructor, Vec<UnsupportedTypeError>> {
    let mut errs = Vec::new();

    let mut inputs = Vec::with_capacity(decl.inputs.len());
    for (arg, ty) in body
        .arguments
        .iter()
        .zip(decl.inputs.iter())
        .skip(1 /* skip ctx */)
    {
        match convert_arg(tcx, &arg.pat, ty) {
            Ok(field) => inputs.push(field),
            Err(err) => errs.push(err),
        }
    }

    if !errs.is_empty() {
        Err(errs)
    } else {
        Ok(StateConstructor { inputs })
    }
}

fn convert_function(
    tcx: TyCtxt,
    name: Symbol,
    decl: &FnDecl,
    body: &Body,
) -> Result<Function, Vec<UnsupportedTypeError>> {
    let mut errs = Vec::new();

    let mutability = match decl.implicit_self {
        hir::ImplicitSelfKind::ImmRef => StateMutability::Immutable,
        hir::ImplicitSelfKind::MutRef => StateMutability::Mutable,
        _ => unreachable!("Syntax pass should have checked RPCs for `self`."),
    };

    let mut inputs = Vec::with_capacity(decl.inputs.len());
    for (arg, ty) in body
        .arguments
        .iter()
        .zip(decl.inputs.iter())
        .skip(2 /* skip self and ctx */)
    {
        match convert_arg(tcx, &arg.pat, ty) {
            Ok(ty) => inputs.push(ty),
            Err(err) => errs.push(err),
        }
    }

    let output = match &decl.output {
        hir::FunctionRetTy::DefaultReturn(_) => None,
        hir::FunctionRetTy::Return(ty) => match convert_ty(tcx, &ty) {
            Ok(Type::Tuple(ref tys)) if tys.is_empty() => None,
            Ok(ty) => Some(ty),
            Err(err) => {
                errs.push(err);
                None
            }
        },
    };

    if !errs.is_empty() {
        Err(errs)
    } else {
        Ok(Function {
            name: name.to_string(),
            mutability,
            inputs,
            output,
        })
    }
}

fn convert_arg(tcx: TyCtxt, pat: &hir::Pat, ty: &hir::Ty) -> Result<Field, UnsupportedTypeError> {
    use hir::PatKind;
    convert_ty(tcx, ty).map(|ty| Field {
        name: match pat.node {
            PatKind::Wild => "_".to_string(),
            PatKind::Binding(_, _, ident, _) => ident.name.as_str().get().to_string(),
            _ => unreachable!("arg pattern must be wild or ident"),
        },
        ty,
    })
}

// this is a macro because it's difficult to convince rustc that `T` \in {`Ty`, `TyS`}`
macro_rules! convert_def {
    ($tcx:ident, $did:expr, $owner_did:expr, $arg_at:expr) => {{
        let (crate_name, def_path_comps) = crate::utils::def_path($tcx, $did);
        let ty_str = def_path_comps.last().cloned().unwrap_or_default();

        Ok(if crate::utils::is_std(crate_name) {
            if ty_str == "String" {
                Type::String
            } else if ty_str == "Vec" {
                match $arg_at(0)? {
                    Type::U8 => Type::Bytes,
                    ty => Type::List(box ty),
                }
            } else if ty_str == "Option" {
                Type::Optional(box $arg_at(0)?)
            } else if ty_str == "Result" {
                Type::Result(box $arg_at(0)?, box $arg_at(1)?)
            } else if ty_str == "HashMap" || ty_str == "BTreeMap" {
                Type::Map(box $arg_at(0)?, box $arg_at(1)?)
            } else if ty_str == "HashSet" || ty_str == "BTreeSet" {
                Type::Set(box $arg_at(0)?)
            } else if ty_str == "Address" {
                Type::Address
            } else {
                // this branch includes `sync`, among other things
                return Err(UnsupportedTypeError::NotReprC(
                    format!("{}::{}", crate_name, def_path_comps.join("::")),
                    $tcx.def_span($owner_did).into(),
                ));
            }
        } else {
            Type::Defined {
                namespace: if crate_name == $tcx.crate_name {
                    None
                } else {
                    Some(crate_name.to_string())
                },
                ty: ty_str,
            }
        })
    }};
}

fn convert_ty(tcx: TyCtxt, ty: &hir::Ty) -> Result<Type, UnsupportedTypeError> {
    use hir::TyKind;
    Ok(match &ty.node {
        TyKind::Slice(ty) => Type::List(box convert_ty(tcx, &ty)?),
        TyKind::Array(ty, len) => {
            let arr_ty = box convert_ty(tcx, &ty)?;
            match tcx.hir().body(len.body).value.node {
                hir::ExprKind::Lit(syntax::source_map::Spanned {
                    node: syntax::ast::LitKind::Int(len, _),
                    ..
                }) => Type::Array(arr_ty, len as u64),
                _ => Type::List(arr_ty),
            }
        }
        TyKind::Rptr(_, hir::MutTy { ty, .. }) => convert_ty(tcx, ty)?,
        TyKind::Tup(tys) => Type::Tuple(
            tys.iter()
                .map(|ty| convert_ty(tcx, ty))
                .collect::<Result<Vec<_>, UnsupportedTypeError>>()?,
        ),
        TyKind::Path(hir::QPath::Resolved(_, path)) => {
            use hir::def::{DefKind, Res};
            let type_args = crate::utils::get_type_args(&path);
            match path.res {
                Res::Def(kind, did) => match kind {
                    DefKind::Struct
                    | DefKind::Union
                    | DefKind::Enum
                    | DefKind::Variant
                    | DefKind::Const => {
                        convert_def!(tcx, did, ty.hir_id.owner_local_def_id().to_def_id(), |i| {
                            convert_ty(tcx, type_args[i])
                        })?
                    }
                    DefKind::TyAlias => {
                        convert_sty_with_arg_at(tcx, did, tcx.type_of(did), |substs, i| {
                            // Use the HIR type parameter, if it exists.
                            // Otherwise, the TyS has extra generics that were defined
                            // in the alias as concrete types (and will be present in the
                            // TyS substs).
                            type_args
                                .get(i)
                                .map(|ty| convert_ty(tcx, ty))
                                .unwrap_or_else(|| convert_sty(tcx, did, substs.type_at(i)))
                        })?
                    }
                    _ => {
                        return Err(UnsupportedTypeError::NotReprC(
                            format!("{:?}", path),
                            path.span.into(),
                        ))
                    }
                },
                Res::PrimTy(ty) => match ty {
                    hir::PrimTy::Int(ty) => convert_int(ty, path.span)?,
                    hir::PrimTy::Uint(ty) => convert_uint(ty, path.span)?,
                    hir::PrimTy::Float(ty) => convert_float(ty, path.span)?,
                    hir::PrimTy::Str => Type::String,
                    hir::PrimTy::Bool => Type::Bool,
                    hir::PrimTy::Char => Type::I8,
                },
                _ => {
                    return Err(UnsupportedTypeError::NotReprC(
                        format!("{:?}", path),
                        path.span.into(),
                    ))
                }
            }
        }
        _ => {
            return Err(UnsupportedTypeError::NotReprC(
                format!("{:?}", ty),
                ty.span.into(),
            ))
        }
    })
}

fn convert_sty<'tcx>(
    tcx: TyCtxt<'tcx>,
    did: DefId,
    ty: &'tcx TyS,
) -> Result<Type, UnsupportedTypeError> {
    convert_sty_with_arg_at(tcx, did, ty, |substs, i| {
        convert_sty(tcx, did, substs.type_at(i))
    })
}

fn convert_sty_with_arg_at<'tcx>(
    tcx: TyCtxt<'tcx>,
    did: DefId,
    ty: &'tcx TyS,
    arg_at: impl Fn(ty::subst::SubstsRef<'tcx>, usize) -> Result<Type, UnsupportedTypeError>,
) -> Result<Type, UnsupportedTypeError> {
    use ty::TyKind::*;
    Ok(match ty.sty {
        Bool => Type::Bool,
        Char => Type::I8,
        Int(ty) => convert_int(ty, tcx.def_span(did))?,
        Uint(ty) => convert_uint(ty, tcx.def_span(did))?,
        Float(ty) => convert_float(ty, tcx.def_span(did))?,
        Adt(AdtDef { did, .. }, substs) => convert_def!(tcx, *did, *did, |i| arg_at(substs, i))?,
        Str => Type::String,
        Array(ty, len) => Type::Array(box convert_sty(tcx, did, ty)?, len.unwrap_usize(tcx)),
        Slice(ty) => Type::List(box convert_sty(tcx, did, ty)?),
        Ref(_, ty, _) => return convert_sty(tcx, did, ty),
        Tuple(substs) => Type::Tuple(
            substs
                .types()
                .map(|ty| convert_sty(tcx, did, ty))
                .collect::<Result<Vec<_>, UnsupportedTypeError>>()?,
        ),
        _ => {
            return Err(UnsupportedTypeError::NotReprC(
                ty.to_string(),
                tcx.def_span(did).into(),
            ))
        }
    })
}

fn convert_int(
    ty: syntax::ast::IntTy,
    span: syntax_pos::Span,
) -> Result<Type, UnsupportedTypeError> {
    use syntax::ast::IntTy;
    Ok(match ty {
        IntTy::I8 => Type::I8,
        IntTy::I16 => Type::I16,
        IntTy::I32 => Type::I32,
        IntTy::I64 => Type::I64,
        IntTy::I128 | IntTy::Isize => {
            return Err(UnsupportedTypeError::NotReprC(ty.to_string(), span.into()))
        }
    })
}

fn convert_uint(
    ty: syntax::ast::UintTy,
    span: syntax_pos::Span,
) -> Result<Type, UnsupportedTypeError> {
    use syntax::ast::UintTy;
    Ok(match ty {
        UintTy::U8 => Type::U8,
        UintTy::U16 => Type::U16,
        UintTy::U32 => Type::U32,
        UintTy::U64 => Type::U64,
        UintTy::U128 | UintTy::Usize => {
            return Err(UnsupportedTypeError::NotReprC(ty.to_string(), span.into()))
        }
    })
}

fn convert_float(
    ty: syntax::ast::FloatTy,
    _span: syntax_pos::Span,
) -> Result<Type, UnsupportedTypeError> {
    use syntax::ast::FloatTy;
    Ok(match ty {
        FloatTy::F32 => Type::F32,
        FloatTy::F64 => Type::F64,
    })
}

fn convert_type_def(
    tcx: TyCtxt,
    def: &AdtDef,
    is_event: bool,
) -> Result<TypeDef, UnsupportedTypeError> {
    let ty_name = tcx
        .def_path(def.did)
        .data
        .iter()
        .last()
        .unwrap()
        .data
        .to_string();
    if def.is_enum() {
        if !def.is_payloadfree() {
            // TODO: convert Rust struct enum to tagged union
            return Err(UnsupportedTypeError::ComplexEnum(
                tcx.def_span(def.did).into(),
            ));
        }
        Ok(TypeDef::Enum {
            name: ty_name,
            variants: def.variants.iter().map(|v| v.ident.to_string()).collect(),
        })
    } else if def.is_struct() {
        let fields = def
            .all_fields()
            .map(|f| {
                Ok((
                    f.ident.to_string(),
                    convert_sty(tcx, f.did, tcx.type_of(f.did))?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(if is_event {
            TypeDef::Event {
                name: ty_name,
                fields: fields
                    .into_iter()
                    .map(|(name, ty)| IndexedField {
                        name,
                        ty,
                        indexed: false,
                    })
                    .collect(),
            }
        } else {
            TypeDef::Struct {
                name: ty_name,
                fields: fields
                    .into_iter()
                    .map(|(name, ty)| Field { name, ty })
                    .collect(),
            }
        })
    } else if def.is_union() {
        // TODO? serde doesn't derive unions. not sure if un-tagged unions are actually useful.
        Err(UnsupportedTypeError::NotReprC(
            def.descr().to_string(),
            tcx.def_span(def.did).into(),
        ))
    } else {
        unreachable!("AdtDef is a struct, enum, or union");
    }
}
