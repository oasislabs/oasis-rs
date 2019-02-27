//! JSON generation

use crate::{items, utils};

/// The result type for JSON errors.
pub type JsonResult<T> = std::result::Result<T, JsonError>;

/// Errors that may occur during JSON operations.
#[derive(Debug)]
pub enum JsonError {
    FailedToCreateDirectory(std::io::Error),
    FailedToCreateJsonFile(std::io::Error),
    FailedToWriteJsonAbiFile(serde_json::Error),
}

impl JsonError {
    /// Returns a JSON error indicating that the creation of the
    /// directory that will contain the JSON file failed.
    pub fn failed_to_create_dir(err: std::io::Error) -> Self {
        JsonError::FailedToCreateDirectory(err)
    }

    /// Returns a JSON error indicating that the creation of the JSON
    /// abi file failed.
    pub fn failed_to_create_json_file(err: std::io::Error) -> Self {
        JsonError::FailedToCreateJsonFile(err)
    }

    /// Returns a JSON error indicating that the writing of the JSON
    /// abi file failed.
    pub fn failed_to_write_json_abi_file(err: serde_json::Error) -> Self {
        JsonError::FailedToWriteJsonAbiFile(err)
    }
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            JsonError::FailedToCreateDirectory(err) => {
                write!(f, "failed to create directory for JSON abi file: {:?}", err)
            }
            JsonError::FailedToCreateJsonFile(err) => {
                write!(f, "failed to create JSON abi file: {:?}", err)
            }
            JsonError::FailedToWriteJsonAbiFile(err) => {
                write!(f, "failed to write JSON abi file: {:?}", err)
            }
        }
    }
}

impl std::error::Error for JsonError {
    fn description(&self) -> &str {
        match self {
            JsonError::FailedToCreateDirectory(_) => {
                "failed to create directory for the JSON abi file"
            }
            JsonError::FailedToCreateJsonFile(_) => "failed to create JSON abi file",
            JsonError::FailedToWriteJsonAbiFile(_) => "failed to write JSON abi file",
        }
    }

    fn cause(&self) -> Option<&std::error::Error> {
        match self {
            JsonError::FailedToCreateDirectory(err) => Some(err),
            JsonError::FailedToCreateJsonFile(err) => Some(err),
            JsonError::FailedToWriteJsonAbiFile(err) => Some(err),
        }
    }
}

/// Writes generated abi JSON file to destination in default target directory.
///
/// # Note
///
/// The generated JSON information may be used by offline tools around WebJS for example.
pub fn write_json_abi(intf: &items::Interface) -> JsonResult<()> {
    use std::{env, fs, path};

    let target = {
        let mut target =
            path::PathBuf::from(env::var("CARGO_TARGET_DIR").unwrap_or(".".to_owned()));
        target.push("target");
        target.push("json");
        fs::create_dir_all(&target).map_err(|err| JsonError::failed_to_create_dir(err))?;
        target.push(&format!("{}.json", intf.name()));
        target
    };

    let mut f =
        fs::File::create(target).map_err(|err| JsonError::failed_to_create_json_file(err))?;

    let abi: Abi = intf.into();

    serde_json::to_writer_pretty(&mut f, &abi)
        .map_err(|err| JsonError::failed_to_write_json_abi_file(err))?;

    Ok(())
}

#[derive(Serialize, Debug)]
pub struct FunctionEntry {
    pub name: String,
    #[serde(rename = "inputs")]
    pub arguments: Vec<Argument>,
    pub outputs: Vec<Argument>,
    pub constant: bool,
}

#[derive(Serialize, Debug)]
pub struct Argument {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Serialize, Debug)]
pub struct ConstructorEntry {
    #[serde(rename = "inputs")]
    pub arguments: Vec<Argument>,
}

#[derive(Serialize, Debug)]
#[serde(tag = "type")]
pub enum AbiEntry {
    #[serde(rename = "event")]
    Event(EventEntry),
    #[serde(rename = "function")]
    Function(FunctionEntry),
    #[serde(rename = "constructor")]
    Constructor(ConstructorEntry),
}

#[derive(Serialize, Debug)]
pub struct EventInput {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub indexed: bool,
}

#[derive(Serialize, Debug)]
pub struct EventEntry {
    pub name: String,
    pub inputs: Vec<EventInput>,
}

#[derive(Serialize, Debug)]
pub struct Abi(pub Vec<AbiEntry>);

impl<'a> From<&'a items::Interface> for Abi {
    fn from(intf: &items::Interface) -> Self {
        let mut result = Vec::new();
        for item in intf.items() {
            match *item {
                items::Item::Event(ref event) => result.push(AbiEntry::Event(event.into())),
                items::Item::Signature(ref signature) => {
                    result.push(AbiEntry::Function(signature.into()))
                }
                _ => {}
            }
        }

        if let Some(constructor) = intf.constructor() {
            result.push(AbiEntry::Constructor(
                FunctionEntry::from(constructor).into(),
            ));
        }

        Abi(result)
    }
}

impl<'a> From<&'a items::Event> for EventEntry {
    fn from(item: &items::Event) -> Self {
        EventEntry {
            name: item.name.to_string(),
            inputs: item
                .indexed
                .iter()
                .map(|&(ref pat, ref ty)| EventInput {
                    name: quote! { #pat }.to_string(),
                    type_: utils::canonicalize_type(ty),
                    indexed: true,
                })
                .chain(item.data.iter().map(|&(ref pat, ref ty)| EventInput {
                    name: quote! { #pat }.to_string(),
                    type_: utils::canonicalize_type(ty),
                    indexed: false,
                }))
                .collect(),
        }
    }
}

impl<'a> From<&'a items::Signature> for FunctionEntry {
    fn from(item: &items::Signature) -> Self {
        FunctionEntry {
            name: item.name.to_string(),
            arguments: item
                .arguments
                .iter()
                .map(|&(ref pat, ref ty)| Argument {
                    name: quote! { #pat }.to_string(),
                    type_: utils::canonicalize_type(ty),
                })
                .collect(),
            outputs: item
                .return_types
                .iter()
                .enumerate()
                .map(|(idx, ty)| Argument {
                    name: format!("returnValue{}", idx),
                    type_: utils::canonicalize_type(ty),
                })
                .collect(),
            constant: item.is_constant,
        }
    }
}

impl From<FunctionEntry> for ConstructorEntry {
    fn from(func: FunctionEntry) -> Self {
        ConstructorEntry {
            arguments: func.arguments,
        }
    }
}
