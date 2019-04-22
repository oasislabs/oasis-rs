//! Types representing an "IR" for RPC interface definitions.

use std::{boxed::Box, collections::BTreeSet};

use rustc::{
    hir::{self, def_id::DefId, FnDecl},
    ty::{self, AdtDef, TyCtxt, TyS},
};

use crate::error::UnsupportedTypeError;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct RpcInterface {
    name: RpcIdent,
    namespace: RpcIdent, // the current crate name
    #[serde(skip_serializing_if = "Vec::is_empty")]
    imports: Vec<RpcImport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    type_defs: Vec<RpcTypeDef>,
    constructor: StateConstructor,
    functions: Vec<RpcFunction>,
    idl_gen_version: String,
}

impl RpcInterface {
    // faq: why return a vec of errors? so that the user can see and correct them all at once.
    pub fn convert(
        tcx: TyCtxt,
        name: syntax_pos::symbol::Ident,
        // the following use BTreeSets to ensure idl is deterministic
        imports: BTreeSet<(syntax_pos::symbol::Symbol, String)>, // (name, version)
        adt_defs: BTreeSet<&AdtDef>,
        fns: &[(syntax_pos::symbol::Ident, &FnDecl)],
    ) -> Result<Self, Vec<UnsupportedTypeError>> {
        let mut errs = Vec::new();

        let imports = imports
            .into_iter()
            .map(|(name, version)| RpcImport {
                name: name.to_string(),
                version,
            })
            .collect();

        let mut type_defs = Vec::with_capacity(adt_defs.len());
        for adt_def in adt_defs.iter() {
            match RpcTypeDef::convert(tcx, adt_def) {
                Ok(type_def) => type_defs.push(type_def),
                Err(err) => errs.push(err),
            }
        }

        let mut ctor = None;
        let mut functions = Vec::with_capacity(fns.len());
        for (name, decl) in fns.iter() {
            if name.as_str() == "new" {
                match StateConstructor::convert(tcx, decl) {
                    Ok(constructor) => ctor = Some(constructor),
                    Err(mut errz) => errs.append(&mut errz),
                }
            } else {
                match RpcFunction::convert(tcx, *name, decl) {
                    Ok(rpc_fn) => functions.push(rpc_fn),
                    Err(mut errz) => errs.append(&mut errz),
                }
            }
        }

        if !errs.is_empty() {
            Err(errs)
        } else {
            Ok(Self {
                name: name.to_string(),
                namespace: tcx.crate_name.to_string(),
                imports,
                type_defs,
                constructor: ctor.unwrap(),
                functions,
                idl_gen_version: env!("CARGO_PKG_VERSION").to_string(),
            })
        }
    }

    pub fn service_name(&self) -> String {
        self.name.to_string()
    }
}

pub type RpcIdent = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct RpcImport {
    name: RpcIdent,
    version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct StateConstructor {
    inputs: Vec<RpcType>,
    // throws: Option<RpcType>,
}

impl StateConstructor {
    fn convert(tcx: TyCtxt, decl: &FnDecl) -> Result<Self, Vec<UnsupportedTypeError>> {
        let mut errs = Vec::new();

        let mut inputs = Vec::with_capacity(decl.inputs.len());
        for inp in decl.inputs.iter().skip(1 /* skip ctx */) {
            match RpcType::convert_ty(tcx, inp) {
                Ok(ty) => inputs.push(ty),
                Err(err) => errs.push(err),
            }
        }

        if !errs.is_empty() {
            Err(errs)
        } else {
            Ok(Self { inputs })
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct RpcFunction {
    name: RpcIdent,
    mutability: StateMutability,
    inputs: Vec<RpcType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<RpcType>,
    // throws: Option<RpcType>,
}

impl RpcFunction {
    fn convert(
        tcx: TyCtxt,
        name: syntax_pos::symbol::Ident,
        decl: &FnDecl,
    ) -> Result<Self, Vec<UnsupportedTypeError>> {
        let mut errs = Vec::new();

        let mutability = match decl.implicit_self {
            hir::ImplicitSelfKind::ImmRef => StateMutability::Immutable,
            hir::ImplicitSelfKind::MutRef => StateMutability::Mutable,
            _ => unreachable!("`#[contract]` should have checked RPCs for `self`."),
        };

        let mut inputs = Vec::with_capacity(decl.inputs.len());
        for inp in decl.inputs.iter().skip(2 /* skip self and ctx */) {
            match RpcType::convert_ty(tcx, inp) {
                Ok(ty) => inputs.push(ty),
                Err(err) => errs.push(err),
            }
        }

        let output = match &decl.output {
            hir::FunctionRetTy::DefaultReturn(_) => None,
            hir::FunctionRetTy::Return(ty) => match &ty.node {
                hir::TyKind::Path(hir::QPath::Resolved(_, path)) => {
                    let result_ty = crate::utils::get_type_args(&path)[0];
                    match RpcType::convert_ty(tcx, &result_ty) {
                        Ok(ret_ty) => match &ret_ty {
                            RpcType::Tuple(tys) if tys.is_empty() => None,
                            _ => Some(ret_ty),
                        },
                        Err(err) => {
                            errs.push(err);
                            None
                        }
                    }
                }
                _ => unreachable!("RPC `fn` must return `Result`"),
            },
        };

        if !errs.is_empty() {
            Err(errs)
        } else {
            Ok(Self {
                name: name.to_string(),
                mutability,
                inputs,
                output,
            })
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum StateMutability {
    Immutable,
    Mutable,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum RpcType {
    Bool,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    Bytes,
    String,
    Address,
    U256,
    H256,
    Defined {
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<RpcIdent>,
        #[serde(rename = "type")]
        ty: RpcIdent,
    },
    Tuple(Vec<RpcType>),
    Array(Box<RpcType>, u64),
    List(Box<RpcType>),
    Set(Box<RpcType>),
    Map(Box<RpcType>, Box<RpcType>),
    Optional(Box<RpcType>),
}

// this is a macro because it's difficult to convince rustc that `T` \in {`Ty`, `TyS`}`
macro_rules! convert_def {
    ($tcx:ident, $did:expr, $converter:expr, $arg_at:expr, $vec_is_bytes:expr) => {{
        let (crate_name, def_path_comps) = crate::utils::def_path($tcx, $did);
        let ty_str = def_path_comps.last().cloned().unwrap_or_default();

        Ok(if crate::utils::is_std(crate_name) {
            if ty_str == "String" {
                RpcType::String
            } else if ty_str == "Vec" {
                let vec_ty = $arg_at(0);
                if $vec_is_bytes(vec_ty) {
                    RpcType::Bytes
                } else {
                    RpcType::List(box $converter($tcx, $did, &vec_ty)?)
                }
            } else if ty_str == "Option" {
                RpcType::Optional(box $converter($tcx, $did, $arg_at(0))?)
            } else if ty_str == "HashMap" || ty_str == "BTreeMap" {
                RpcType::Map(
                    box $converter($tcx, $did, $arg_at(0))?,
                    box $converter($tcx, $did, $arg_at(1))?,
                )
            } else if ty_str == "HashSet" || ty_str == "BTreeSet" {
                RpcType::Set(box $converter($tcx, $did, $arg_at(0))?)
            } else if ty_str == "Address" || ty_str == "H160" {
                RpcType::Address
            } else if ty_str == "U256" {
                RpcType::U256
            } else if ty_str == "H256" {
                RpcType::H256
            } else {
                // this branch includes `sync`, among other things
                return Err(UnsupportedTypeError::NotReprC(
                    format!("{}::{}", crate_name, def_path_comps.join("::")),
                    $tcx.def_span($did),
                ));
            }
        } else {
            RpcType::Defined {
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

impl RpcType {
    fn convert_ty(tcx: TyCtxt, ty: &hir::Ty) -> Result<Self, UnsupportedTypeError> {
        use hir::TyKind;
        Ok(match &ty.node {
            TyKind::Slice(ty) => RpcType::List(box RpcType::convert_ty(tcx, &ty)?),
            TyKind::Array(ty, len) => {
                let arr_ty = box RpcType::convert_ty(tcx, &ty)?;
                match tcx.hir().body(len.body).value.node {
                    hir::ExprKind::Lit(syntax::source_map::Spanned {
                        node: syntax::ast::LitKind::Int(len, _),
                        ..
                    }) => RpcType::Array(arr_ty, len as u64),
                    _ => RpcType::List(arr_ty),
                }
            }
            TyKind::Rptr(_, hir::MutTy { ty, .. }) => RpcType::convert_ty(tcx, ty)?,
            TyKind::Tup(tys) => RpcType::Tuple(
                tys.iter()
                    .map(|ty| RpcType::convert_ty(tcx, ty))
                    .collect::<Result<Vec<_>, UnsupportedTypeError>>()?,
            ),
            TyKind::Path(hir::QPath::Resolved(_, path)) => {
                use hir::def::Def;
                match path.def {
                    Def::Struct(did)
                    | Def::Union(did)
                    | Def::Enum(did)
                    | Def::Variant(did)
                    | Def::TyAlias(did)
                    | Def::Const(did) => {
                        let type_args = crate::utils::get_type_args(&path);
                        let is_vec_u8 = |vec_ty: &hir::Ty| match vec_ty.node {
                            hir::TyKind::Path(hir::QPath::Resolved(_, ref path))
                                if path.to_string() == "u8" =>
                            {
                                true
                            }
                            _ => false,
                        };
                        convert_def!(
                            tcx,
                            did,
                            |tcx, _, ty| RpcType::convert_ty(tcx, ty),
                            |i| { type_args[i] },
                            is_vec_u8
                        )?
                    }
                    Def::PrimTy(ty) => match ty {
                        hir::PrimTy::Int(ty) => RpcType::convert_int(ty, path.span)?,
                        hir::PrimTy::Uint(ty) => RpcType::convert_uint(ty, path.span)?,
                        hir::PrimTy::Float(ty) => RpcType::convert_float(ty, path.span)?,
                        hir::PrimTy::Str => RpcType::String,
                        hir::PrimTy::Bool => RpcType::Bool,
                        hir::PrimTy::Char => RpcType::I8,
                    },
                    _ => {
                        return Err(UnsupportedTypeError::NotReprC(path.to_string(), path.span));
                    }
                }
            }
            _ => return Err(UnsupportedTypeError::NotReprC(format!("{:?}", ty), ty.span)),
        })
    }

    fn convert_sty(tcx: TyCtxt, did: DefId, ty: &TyS) -> Result<Self, UnsupportedTypeError> {
        use ty::TyKind::*;
        Ok(match ty.sty {
            Bool => RpcType::Bool,
            Char => RpcType::I8,
            Int(ty) => RpcType::convert_int(ty, tcx.def_span(did))?,
            Uint(ty) => RpcType::convert_uint(ty, tcx.def_span(did))?,
            Float(ty) => RpcType::convert_float(ty, tcx.def_span(did))?,
            Adt(AdtDef { did, .. }, substs) => {
                let is_vec_u8 = |vec_ty: &TyS| {
                    if let Uint(syntax::ast::UintTy::U8) = vec_ty.sty {
                        true
                    } else {
                        false
                    }
                };
                convert_def!(
                    tcx,
                    *did,
                    &RpcType::convert_sty,
                    |i| substs.type_at(i),
                    is_vec_u8
                )?
            }
            Str => RpcType::String,
            Array(ty, len) => RpcType::Array(
                box RpcType::convert_sty(tcx, did, ty)?,
                len.unwrap_usize(tcx),
            ),
            Slice(ty) => RpcType::List(box RpcType::convert_sty(tcx, did, ty)?),
            Ref(_, ty, _) => return RpcType::convert_sty(tcx, did, ty),
            Tuple(tys) => RpcType::Tuple(
                tys.iter()
                    .map(|ty| RpcType::convert_sty(tcx, did, ty))
                    .collect::<Result<Vec<_>, UnsupportedTypeError>>()?,
            ),
            _ => {
                return Err(UnsupportedTypeError::NotReprC(
                    ty.to_string(),
                    tcx.def_span(did),
                ))
            }
        })
    }

    fn convert_int(
        ty: syntax::ast::IntTy,
        span: syntax_pos::Span,
    ) -> Result<RpcType, UnsupportedTypeError> {
        use syntax::ast::IntTy;
        Ok(match ty {
            IntTy::I8 => RpcType::I8,
            IntTy::I16 => RpcType::I16,
            IntTy::I32 => RpcType::I32,
            IntTy::I64 => RpcType::I64,
            IntTy::I128 | IntTy::Isize => {
                return Err(UnsupportedTypeError::NotReprC(ty.to_string(), span))
            }
        })
    }

    fn convert_uint(
        ty: syntax::ast::UintTy,
        span: syntax_pos::Span,
    ) -> Result<RpcType, UnsupportedTypeError> {
        use syntax::ast::UintTy;
        Ok(match ty {
            UintTy::U8 => RpcType::U8,
            UintTy::U16 => RpcType::U16,
            UintTy::U32 => RpcType::U32,
            UintTy::U64 => RpcType::U64,
            UintTy::U128 | UintTy::Usize => {
                return Err(UnsupportedTypeError::NotReprC(ty.to_string(), span))
            }
        })
    }

    fn convert_float(
        ty: syntax::ast::FloatTy,
        _span: syntax_pos::Span,
    ) -> Result<RpcType, UnsupportedTypeError> {
        use syntax::ast::FloatTy;
        Ok(match ty {
            FloatTy::F32 => RpcType::F32,
            FloatTy::F64 => RpcType::F64,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum RpcTypeDef {
    Struct {
        name: RpcIdent,
        fields: Vec<RpcField>,
    },
    Enum {
        name: RpcIdent,
        variants: Vec<RpcIdent>,
    },
    // TODO: unions and exceptions
}

impl RpcTypeDef {
    fn convert(tcx: TyCtxt, def: &AdtDef) -> Result<Self, UnsupportedTypeError> {
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
                return Err(UnsupportedTypeError::ComplexEnum(tcx.def_span(def.did)));
            }
            Ok(RpcTypeDef::Enum {
                name: ty_name,
                variants: def.variants.iter().map(|v| v.ident.to_string()).collect(),
            })
        } else if def.is_struct() {
            let fields = def
                .all_fields()
                .map(|f| {
                    Ok(RpcField {
                        name: f.ident.to_string(),
                        ty: RpcType::convert_sty(tcx, f.did, tcx.type_of(f.did))?,
                    })
                })
                .collect::<Result<Vec<RpcField>, UnsupportedTypeError>>()?;
            Ok(RpcTypeDef::Struct {
                name: ty_name,
                fields,
            })
        } else if def.is_union() {
            // TODO? serde doesn't derive unions. not sure if un-tagged unions are actually useful.
            Err(UnsupportedTypeError::NotReprC(
                def.descr().to_string(),
                tcx.def_span(def.did),
            ))
        } else {
            unreachable!("AdtDef must be struct, enum, or union");
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct RpcField {
    name: RpcIdent,
    #[serde(rename = "type")]
    ty: RpcType,
}