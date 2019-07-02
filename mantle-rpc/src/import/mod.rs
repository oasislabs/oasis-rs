mod error;
mod file_importer;

pub use error::ImporterError;

pub struct Importer {
    backend: Box<dyn ImporterBackend>,
}

impl Importer {
    pub fn for_url(url_str: &str) -> Result<Self, ImporterError> {
        let url = match url::Url::parse(&url_str) {
            Ok(url) => url,
            Err(err) => return Err(ImporterError::InvalidUrl(err)),
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
                            ImporterError::Fail(failure::format_err!(
                                "Could not determine file path for URL: `{}`",
                                url_str
                            ))
                        })?
                    } else {
                        let relpath_str =
                            &url_str[(path_start + 1)..(url_path_start + url.path().len())];
                        std::path::Path::new(relpath_str)
                            .canonicalize()
                            .map_err(|err| {
                                ImporterError::Fail(failure::format_err!(
                                    "{}: {}",
                                    err,
                                    relpath_str
                                ))
                            })?
                    };
                    box file_importer::FileImporter { path }
                }
                _ => return Err(ImporterError::NoImporter(url.scheme().to_string())),
            },
        })
    }

    pub fn import(&self, name: &str) -> Result<ImportedService, ImporterError> {
        self.backend.import(name)
    }

    pub fn import_all(&self) -> Result<Vec<ImportedService>, ImporterError> {
        self.backend.import_all()
    }
}

pub struct ImportedService {
    pub bytecode: Vec<u8>,
    pub interface: crate::Interface,
}

trait ImporterBackend {
    fn import(&self, name: &str) -> Result<ImportedService, ImporterError>;

    fn import_all(&self) -> Result<Vec<ImportedService>, ImporterError>;
}
