#![feature(box_syntax)]

use std::io::{Read, Write};

#[macro_use]
extern crate serde;

#[cfg(feature = "import")]
pub mod import;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
pub struct Interface {
    pub name: Ident,
    pub namespace: Ident, // the current crate name
    pub version: Ident,   // semver
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub imports: Vec<Import>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub type_defs: Vec<TypeDef>,
    pub constructor: Constructor,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub functions: Vec<Function>,
    pub oasis_build_version: String,
}

#[cfg(feature = "saveload")]
impl Interface {
    pub fn from_reader(rd: impl Read) -> Result<Self, failure::Error> {
        Ok(serde_json::from_reader(xz2::read::XzDecoder::new(rd))?)
    }

    pub fn to_writer(&self, wr: impl Write) -> Result<(), failure::Error> {
        let mut encoder = xz2::write::XzEncoder::new(wr, 9 /* max compression level */);
        serde_json::to_writer(&mut encoder, self)?;
        Ok(encoder.try_finish()?)
    }

    pub fn from_slice(sl: &[u8]) -> Result<Self, failure::Error> {
        Self::from_reader(sl)
    }

    pub fn to_vec(&self) -> Result<Vec<u8>, failure::Error> {
        let mut bytes = Vec::new();
        self.to_writer(&mut bytes)?;
        Ok(bytes)
    }

    pub fn to_string(&self) -> Result<String, failure::Error> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

pub type Ident = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
pub struct Function {
    pub name: Ident,
    pub mutability: StateMutability,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub inputs: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output: Option<Type>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
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

impl TypeDef {
    pub fn name(&self) -> &str {
        match self {
            TypeDef::Struct { name, .. }
            | TypeDef::Enum { name, .. }
            | TypeDef::Event { name, .. } => &name,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
pub struct Field {
    pub name: Ident,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
pub struct IndexedField {
    pub name: Ident,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub indexed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
#[serde(rename_all = "lowercase")]
pub enum StateMutability {
    Immutable,
    Mutable,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
pub struct Import {
    pub name: Ident,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub registry: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
pub struct Constructor {
    pub inputs: Vec<Field>,
    pub error: Option<Type>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Hash)]
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
        #[serde(skip_serializing_if = "Option::is_none", default)]
        namespace: Option<Ident>, // `None` if local, otherwise refers to an entry in `Imports`
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
