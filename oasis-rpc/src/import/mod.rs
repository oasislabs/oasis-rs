mod file_importer;
#[cfg(feature = "resolve")]
mod resolve;

#[cfg(feature = "resolve")]
pub use resolve::{resolve_imports, ResolveError};

pub struct Importer {
    backend: Box<dyn ImporterBackend>,
}

impl Importer {
    pub fn for_location(
        location: ImportLocation,
        base_dir: &std::path::Path,
    ) -> Result<Self, ImportError> {
        match location {
            ImportLocation::Path(path) => {
                let abs_path = if path.is_relative() {
                    base_dir.join(&path)
                } else {
                    path.clone()
                };
                abs_path
                    .canonicalize()
                    .map(|path| Self {
                        backend: box file_importer::FileImporter { path },
                    })
                    .map_err(|_| ImportError::NoImport(path.display().to_string()))
            }
            ImportLocation::Url(url) => Ok(Self {
                backend: match url.scheme() {
                    "file" => box file_importer::FileImporter {
                        path: std::path::PathBuf::from(url.path()),
                    },
                    _ => return Err(ImportError::NoImporter(url.scheme().to_string())),
                },
            }),
        }
    }

    pub fn import(&self, name: &str) -> Result<ImportedService, ImportError> {
        self.backend.import(name)
    }

    pub fn import_all(&self) -> Result<Vec<ImportedService>, ImportError> {
        self.backend.import_all()
    }
}

pub enum ImportLocation {
    Path(std::path::PathBuf),
    Url(url::Url),
    // Address(oasis_types::Address),
}

impl<'de> serde::Deserialize<'de> for ImportLocation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "lowercase", untagged)]
        enum TomlImportLocation {
            Path { path: std::path::PathBuf },
            Url { url: url::Url },
        }
        Ok(match TomlImportLocation::deserialize(deserializer)? {
            TomlImportLocation::Path { path } => ImportLocation::Path(path),
            TomlImportLocation::Url { url } => ImportLocation::Url(url),
        })
    }
}

pub struct ImportedService {
    pub bytecode: Vec<u8>,
    pub interface: crate::Interface,
}

trait ImporterBackend {
    fn import(&self, name: &str) -> Result<ImportedService, ImportError>;

    fn import_all(&self) -> Result<Vec<ImportedService>, ImportError>;
}

#[derive(Debug, failure::Fail)]
pub enum ImportError {
    #[fail(display = "could not import from `{}`: {}", _0, _1)]
    Io(String /* resource */, #[fail(cause)] std::io::Error),

    #[fail(display = "no importer for {} URL scheme", _0)]
    NoImporter(String),

    #[fail(display = "wasm module missing oasis-interface section")]
    MissingInterfaceSection,

    #[fail(display = "could not locate `{}`", _0)]
    NoImport(String),

    #[fail(display = "{}", _0)]
    Importer(#[fail(cause)] failure::Error),
}
