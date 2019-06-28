#[macro_use]
extern crate serde;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct Interface {
    pub name: Ident,
    pub namespace: Ident, // the current crate name
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub imports: Vec<Import>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub type_defs: Vec<TypeDef>,
    pub constructor: StateConstructor,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub functions: Vec<Function>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub has_default_function: bool,
    pub mantle_build_version: String,
}

pub type Ident = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
pub struct Function {
    pub name: Ident,
    pub mutability: StateMutability,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub inputs: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Type>,
    // throws: Option<Type>,
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
    // TODO: unions and exceptions
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
pub struct StateConstructor {
    pub inputs: Vec<Field>,
    // throws: Option<Type>,
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
}
