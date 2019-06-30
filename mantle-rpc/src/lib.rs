#![feature(box_syntax)]

#[macro_use]
extern crate serde;

#[cfg(feature = "importer")]
mod importer;
#[cfg(feature = "importer")]
pub use importer::Importer;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct Interface {
    pub name: Ident,
    pub namespace: Ident, // the current crate name
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub imports: Vec<Import>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub type_defs: Vec<TypeDef>,
    pub constructor: Constructor,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub functions: Vec<Function>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub has_default_function: bool,
    pub mantle_build_version: String,
}

#[cfg(feature = "saveload")]
impl Interface {
    pub fn from_slice(sl: &[u8]) -> Result<crate::Interface, failure::Error> {
        use std::io::Read as _;
        let mut decoder = libflate::deflate::Decoder::new(sl);
        let mut inflated = Vec::new();
        decoder.read_to_end(&mut inflated)?;
        Ok(serde_json::from_slice(&inflated)?)
    }

    pub fn to_vec(&self) -> Result<Vec<u8>, failure::Error> {
        let mut encoder = libflate::deflate::Encoder::new(Vec::new());
        serde_json::to_writer(&mut encoder, self)?;
        Ok(encoder.finish().into_result()?)
    }

    pub fn to_string(&self) -> Result<String, failure::Error> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

pub type Ident = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct Function {
    pub name: Ident,
    pub mutability: StateMutability,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub inputs: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output: Option<Type>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum TypeDef {
    Struct {
        name: Ident,
        fields: Vec<Field>,
    },
    Enum {
        name: Ident,
        variants: Vec<Ident>,
    },
    Event {
        name: Ident,
        fields: Vec<IndexedField>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct Field {
    pub name: Ident,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct IndexedField {
    pub name: Ident,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub indexed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum StateMutability {
    Immutable,
    Mutable,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct Import {
    pub name: Ident,
    pub version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct Constructor {
    pub inputs: Vec<Field>,
    pub error: Option<Type>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase", tag = "type", content = "params")]
pub enum Type {
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
    Defined {
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<Ident>,
        #[serde(rename = "type")]
        ty: Ident,
    },
    Tuple(Vec<Type>),
    Array(Box<Type>, u64),
    List(Box<Type>),
    Set(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Optional(Box<Type>),
    Result(Box<Type>, Box<Type>),
}
