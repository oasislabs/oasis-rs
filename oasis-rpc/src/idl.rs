#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub oasis_build_version: Option<String>,
}

pub type Ident = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct Function {
    pub name: Ident,
    pub mutability: StateMutability,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub inputs: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output: Option<Type>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum TypeDef {
    Struct {
        name: Ident,
        fields: Vec<Field>,
    },
    Enum {
        name: Ident,
        variants: Vec<EnumVariant>,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct Field {
    pub name: Ident,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct EnumVariant {
    pub name: Ident,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fields: Option<EnumFields>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
#[serde(untagged)]
pub enum EnumFields {
    Named(Vec<Field>),
    Tuple(Vec<Type>),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct IndexedField {
    pub name: Ident,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub indexed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
#[serde(rename_all = "lowercase")]
pub enum StateMutability {
    Immutable,
    Mutable,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct Import {
    pub name: Ident,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub registry: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct Constructor {
    pub inputs: Vec<Field>,
    pub error: Option<Type>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
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
    Balance,
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
