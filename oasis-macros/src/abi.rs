use std::convert::TryFrom;

use crate::RPC;

/// Generates an interface definition for the provided RPCs.
///
/// v1.5: Writes `<contract_ident>.json` to a directory specified by the `ABI_DIR` env var.
///       `ABI_DIR` should be an apsolute path set by `oasis_std::build::build_contract`.
pub(crate) fn generate(
    contract_ident: &syn::Ident,
    ctor: &RPC,
    rpcs: &[RPC],
) -> Result<(), std::io::Error> {
    let abi_defs = std::iter::once(ctor)
        .chain(rpcs.into_iter())
        .map(|rpc| rpc.into())
        .collect::<Vec<AbiEntry>>();

    let mut json_path = std::path::PathBuf::from(
        std::env::var_os("ABI_DIR").expect("Build script should have set `ABI_DIR`"),
    );
    json_path.push(format!("{}.json", contract_ident));

    std::fs::write(json_path, serde_json::to_string_pretty(&abi_defs)?)
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

impl<'a> From<&'a syn::Type> for Param {
    /// Converts a Rust type to an `AbiParam`. Panics if conversion is unsupported.
    fn from(ty: &syn::Type) -> Self {
        use syn::{Type::*, *};
        let mut components = vec![];
        let param_type = match ty {
            Array(_) | Reference(_) => AbiType::try_from(ty)
                .expect(&format!("Could not map `{:?}` to Ethereum ABI type", ty)),
            Tuple(tup) => {
                components = tup.elems.iter().map(|el| el.into()).collect();
                AbiType::Tuple
            }
            Path(TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => match AbiType::try_from(ty) {
                Ok(abi_ty) => abi_ty,
                Err(_) => {
                    unimplemented!("look up defintion of `{:?}` and convert to tuple", segments)
                }
            },
            Paren(TypeParen { box elem, .. }) => return Param::from(elem),
            Group(TypeGroup { box elem, .. }) => return Param::from(elem),
            _ => panic!("Could not map `{:?}` to Ethereum ABI type", ty),
        };
        Self {
            name: None,
            param_type,
            components,
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
    type Error = String;
    fn try_from(ty: &syn::Type) -> Result<Self, Self::Error> {
        use syn::{Type::*, *};
        Ok(match ty {
            Array(TypeArray { box elem, len, .. }) => AbiType::Array {
                ty: box Self::try_from(elem)?,
                len: match len {
                    Expr::Lit(ExprLit {
                        lit: Lit::Int(lit_int),
                        ..
                    }) => lit_int.value() as usize,
                    _ => return Err(format!("Invalid array len `{:?}`", len)),
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
                                    AbiType::Vec(box Param::from(ty).param_type)
                                }
                            }
                            _ => unreachable!("`Vec` must have type parameter"),
                        },
                        _ => unreachable!("`Vec` can only have one angle bracketed type parameter"),
                    },
                    ty => panic!("`{}` could not be converted to Ethereum ABI type", ty),
                }
            }
            Paren(TypeParen { box elem, .. }) => Self::try_from(elem)?,
            Group(TypeGroup { box elem, .. }) => Self::try_from(elem)?,
            _ => return Err(format!("Could not map `{:?}` to Ethereum ABI type", ty)),
        })
    }
}
impl<'a, 'r> From<&'a RPC<'r>> for AbiEntry {
    fn from(rpc: &RPC) -> Self {
        let (entry_type, name, state_mutability, outputs) = if rpc.is_ctor() {
            (EntryType::Constructor, None, StateMutability::Payable, None)
        } else {
            let mutability = if rpc.is_mut() {
                StateMutability::Payable
            } else {
                StateMutability::View
            };

            let outputs = match rpc.result_ty() {
                syn::Type::Tuple(tup) => tup.elems.iter().map(|out| out.into()).collect(),
                out => vec![Param::from(out)],
            };
            (
                EntryType::Function,
                Some(rpc.sig.ident.to_string()),
                mutability,
                Some(outputs),
            )
        };

        let inputs = rpc
            .inputs
            .iter()
            .map(|(pat, ty)| {
                let mut ty = Param::from(*ty);
                ty.name = match pat {
                    syn::Pat::Ident(syn::PatIdent { ident, .. }) => Some(ident.to_string()),
                    _ => unreachable!("Captured function args always have ident in Rust 2018"),
                };
                ty
            })
            .collect();

        Self {
            entry_type,
            name,
            inputs,
            outputs,
            state_mutability,
        }
    }
}
