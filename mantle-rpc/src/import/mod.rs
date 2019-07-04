mod file_importer;
#[cfg(feature = "resolve")]
mod resolve;

#[cfg(feature = "resolve")]
pub use resolve::{ResolveError, Resolver};

pub struct Importer {
    backend: Box<dyn ImporterBackend>,
}

impl Importer {
    pub fn for_url(url_str: &str, mut base_dir: std::path::PathBuf) -> Result<Self, ImportError> {
        let url = match url::Url::parse(&url_str) {
            Ok(url) => url,
            Err(err) => return Err(ImportError::InvalidUrl(err)),
        };
        Ok(Self {
            backend: match url.scheme() {
                "file" => {
                    let url_path_start = url_str.find(url.path()).unwrap();
                    let path_start = url_str
                        .match_indices('/')
                        .nth(2)
                        .map(|ind| ind.0)
                        .unwrap_or(url_path_start);
                    let path = if path_start == url_path_start {
                        // absolute path
                        url.to_file_path().map_err(|_| {
                            ImportError::Importer(failure::format_err!(
                                "Could not determine file path for URL: `{}`",
                                url_str
                            ))
                        })?
                    } else {
                        let relpath_str =
                            &url_str[(path_start + 1)..(url_path_start + url.path().len())];
                        base_dir.push(relpath_str);
                        base_dir
                            .canonicalize()
                            .map_err(|err| ImportError::Io(relpath_str.to_string(), err))?
                    };
                    box file_importer::FileImporter { path }
                }
                _ => return Err(ImportError::NoImporter(url.scheme().to_string())),
            },
        })
    }

    pub fn import(&self, name: &str) -> Result<ImportedService, ImportError> {
        self.backend.import(name)
    }

    pub fn import_all(&self) -> Result<Vec<ImportedService>, ImportError> {
        self.backend.import_all()
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
    #[fail(display = "Invalid URL: {}", _0)]
    InvalidUrl(#[fail(cause)] url::ParseError),

    #[fail(display = "Could not import from `{}`: {}", _0, _1)]
    Io(String /* resource */, #[fail(cause)] std::io::Error),

    #[fail(display = "No importer for scheme `{}`", _0)]
    NoImporter(String),

    #[fail(display = "Wasm module missing mantle-interface section")]
    MissingInterfaceSection,

    #[fail(display = "Could not locate `{}`", _0)]
    NoImport(String),

    #[fail(display = "{}", _0)]
    Importer(#[fail(cause)] failure::Error),
}
