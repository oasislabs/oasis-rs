use std::convert::TryFrom;

use syn::visit_mut::VisitMut;

use crate::RPC;

/// Generates an interface definition for the provided RPCs.
///
/// v1.5: Writes `<contract_ident>.json` to a directory specified by the `ABI_DIR` env var.
///       `ABI_DIR`, if provided, is an abspath set by `oasis_std::build::build_contract`.
///       The ABI will not be generated if `ABI_DIR` is absent.
pub(crate) fn generate<'a>(
    contract_ident: &'a syn::Ident,
    ctor: &'a RPC,
    rpcs: &'a [RPC],
) -> Result<(), Vec<&'a (dyn syn::spanned::Spanned)>> {
    let abi_dir = std::env::var_os("ABI_DIR");
    if abi_dir.is_none() {
        return Ok(());
    }
    let mut abi_defs = Vec::with_capacity(rpcs.len() + 1);
    let mut errs = Vec::new();
    for rpc in std::iter::once(ctor).chain(rpcs.into_iter()) {
        match AbiEntry::try_from(rpc) {
            Ok(entry) => abi_defs.push(entry),
            Err(mut err) => errs.append(&mut err),
        }
    }

    if !errs.is_empty() {
        Err(errs)
    } else {
        let mut json_path = std::path::PathBuf::from(abi_dir.unwrap());
        json_path.push(format!("{}.json", contract_ident));
        std::fs::write(
            json_path,
            serde_json::to_string_pretty(&abi_defs).expect("Could not serialize ABI"),
        )
        .expect("Could not write ABI JSON");
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AbiEntry {
    #[serde(rename = "type")]
    entry_type: EntryType,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>, // `None` when constructor
    inputs: Vec<Param>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<Vec<Param>>, // `None` when constructor
    state_mutability: StateMutability,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum EntryType {
    Function,
    Constructor,
    // Fallback, // TODO?
}

#[derive(Serialize)]
struct Param {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>, // `None` when output value
    #[serde(rename = "type")]
    param_type: AbiType,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<Param>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum StateMutability {
    // Pure, // TODO?
    View,
    // Nonpayable, // TODO?
    Payable,
}

enum AbiType {
    UInt(usize), // 8 <= bits <= 256; bits % 8 = 0
    Int(usize),  // 8 <= bits <= 256; bits % 8 = 0
    Address,
    Bool,
    Tuple,
    Array { ty: Box<AbiType>, len: usize },
    Bytes,
    String,
    Vec(Box<AbiType>),
    // the following don't exist in Rust
    // Fixed {
    //     bits: usize,     // 8 <= bits <= 256; bits % 8 = 0
    //     exponent: usize, // 0 <= exponent <= 80
    // },
    // FixedBytes {
    //     num_bytes: usize, // 0 < num_bytes <= 32
    // },
    // Function {
    //     address: [u8; 20],
    //     selector: [u8; 4],
    // },
}

impl<'a> TryFrom<&'a syn::Type> for Param {
    type Error = Vec<&'a (dyn syn::spanned::Spanned)>;
    fn try_from(ty: &'a syn::Type) -> Result<Self, Self::Error> {
        use syn::{Type::*, *};
        let mut components = vec![];
        let mut errs = vec![];
        let param_type = match ty {
            Array(_) | Reference(_) => match AbiType::try_from(ty) {
                Ok(param_ty) => param_ty,
                Err(err) => return Err(vec![err]),
            },
            Tuple(tup) => {
                for el in tup.elems.iter() {
                    match Param::try_from(el) {
                        Ok(param) => components.push(param),
                        Err(mut errz) => errs.append(&mut errz),
                    }
                }
                AbiType::Tuple
            }
            Path(_) => match AbiType::try_from(ty) {
                Ok(abi_ty) => abi_ty,
                Err(err) => return Err(vec![err]),
            },
            Paren(TypeParen { box elem, .. }) => return Param::try_from(elem),
            Group(TypeGroup { box elem, .. }) => return Param::try_from(elem),
            _ => return Err(vec![ty]),
        };
        if !errs.is_empty() {
            Err(errs)
        } else {
            Ok(Self {
                name: None,
                param_type,
                components,
            })
        }
    }
}

impl AbiType {
    fn to_string(&self) -> String {
        use AbiType::*;
        match self {
            UInt(bits) => format!("uint{}", bits),
            Int(bits) => format!("int{}", bits),
            Address => "address".to_string(),
            Bool => "bool".to_string(),
            Tuple => "tuple".to_string(),
            Array { ty, len } => format!("{}[{}]", ty.to_string(), len),
            Bytes => "bytes".to_string(),
            String => "string".to_string(),
            Vec(ty) => format!("{}[]", ty.to_string()),
        }
    }
}

impl serde::Serialize for AbiType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'a> TryFrom<&'a syn::Type> for AbiType {
    type Error = &'a (dyn syn::spanned::Spanned);
    fn try_from(ty: &'a syn::Type) -> Result<Self, Self::Error> {
        use syn::{Type::*, *};
        Ok(match ty {
            Array(TypeArray { box elem, len, .. }) => AbiType::Array {
                ty: box Self::try_from(elem)?,
                len: match len {
                    Expr::Lit(ExprLit {
                        lit: Lit::Int(lit_int),
                        ..
                    }) => lit_int.value() as usize,
                    _ => return Err(len),
                },
            },
            Reference(TypeReference { box elem, .. }) => Self::try_from(elem)?,
            Path(TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => {
                let seg = segments.iter().last().unwrap();
                match seg.ident.to_string().as_str() {
                    "bool" => AbiType::Bool,
                    "char" => AbiType::Int(8),

                    "u8" => AbiType::UInt(8),
                    "u16" => AbiType::UInt(16),
                    "u32" => AbiType::UInt(32),
                    "u64" => AbiType::UInt(64),
                    "u128" => AbiType::UInt(128),
                    "usize" => AbiType::UInt(32),

                    "i8" => AbiType::Int(8),
                    "i16" => AbiType::Int(16),
                    "i32" => AbiType::Int(32),
                    "i64" => AbiType::Int(64),
                    "i128" => AbiType::Int(128),
                    "isize" => AbiType::Int(32),

                    "H160" => AbiType::UInt(160),
                    "U256" | "H256" => AbiType::UInt(256),
                    "Address" => AbiType::Address,

                    "String" => AbiType::String,
                    "Vec" => match &seg.arguments {
                        PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            args,
                            ..
                        }) => match args.iter().nth(0).expect("`Vec` must have type parameter") {
                            GenericArgument::Type(ty) => {
                                if *ty == parse_quote!(u8): syn::Type {
                                    AbiType::Bytes
                                } else {
                                    AbiType::Vec(box AbiType::try_from(ty)?)
                                }
                            }
                            _ => unreachable!("`Vec` must have type parameter"),
                        },
                        _ => unreachable!("`Vec` can only have one angle bracketed type parameter"),
                    },
                    _ => return Err(segments),
                }
            }
            Paren(TypeParen { box elem, .. }) => Self::try_from(elem)?,
            Group(TypeGroup { box elem, .. }) => Self::try_from(elem)?,
            _ => return Err(ty),
        })
    }
}
impl<'a, 'r> TryFrom<&'a RPC<'r>> for AbiEntry {
    type Error = Vec<&'a (dyn syn::spanned::Spanned)>;
    fn try_from(rpc: &'a RPC) -> Result<Self, Self::Error> {
        let mut errs = Vec::new();
        let (entry_type, name, state_mutability, outputs) = if rpc.is_ctor() {
            (EntryType::Constructor, None, StateMutability::Payable, None)
        } else {
            let mutability = if rpc.is_mut() {
                StateMutability::Payable
            } else {
                StateMutability::View
            };

            let mut owned_result_ty = rpc.result_ty().clone();
            crate::Deborrower {}.visit_type_mut(&mut owned_result_ty);

            let mut outputs = Vec::new();
            // In the following section, both the original and deborrowed types
            // are simultaneously tracked so that, if the deborrowed type is unconvertable,
            // the spans for the user's types can be returned.
            match (rpc.result_ty(), owned_result_ty) {
                (syn::Type::Tuple(tup), syn::Type::Tuple(ref owned_tup)) => {
                    for (el, owned_el) in tup.elems.iter().zip(owned_tup.elems.iter()) {
                        match Param::try_from(owned_el) {
                            Ok(el) => outputs.push(el),
                            Err(_) => errs.append(&mut Param::try_from(el).err().unwrap()),
                        }
                    }
                }
                (out, _) => match Param::try_from(out) {
                    Ok(out) => outputs.push(out),
                    Err(mut errz) => errs.append(&mut errz),
                },
            };
            (
                EntryType::Function,
                Some(rpc.sig.ident.to_string()),
                mutability,
                Some(outputs),
            )
        };

        let mut inputs = Vec::with_capacity(rpc.inputs.len());
        for (pat, ty) in rpc.inputs.iter() {
            match Param::try_from(*ty) {
                Ok(mut ty) => {
                    ty.name = match pat {
                        syn::Pat::Ident(syn::PatIdent { ident, .. }) => Some(ident.to_string()),
                        _ => unreachable!("Captured function args always have ident in Rust 2018"),
                    };
                    inputs.push(ty)
                }
                Err(mut errz) => errs.append(&mut errz),
            }
        }

        if !errs.is_empty() {
            Err(errs)
        } else {
            Ok(Self {
                entry_type,
                name,
                inputs,
                outputs,
                state_mutability,
            })
        }
    }
}
