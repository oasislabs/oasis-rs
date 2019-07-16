pub struct Resolver {
    deps: Vec<(String, String)>,
    base_dir: std::path::PathBuf,
}

impl Resolver {
    pub fn new(initial_deps: Vec<(String, String)>, base_dir: std::path::PathBuf) -> Self {
        Self {
            deps: initial_deps,
            base_dir,
        }
    }

    pub fn resolve(&self) -> Result<Vec<super::ImportedService>, ResolveError> {
        self.deps
            .iter()
            .map(|(name, url)| {
                super::Importer::for_url(url, self.base_dir.clone())
                    .and_then(|importer| importer.import(name))
                    .map_err(ResolveError::Import)
            })
            .collect()
    }
}

#[derive(Debug, failure::Fail)]
pub enum ResolveError {
    Import(#[fail(cause)] super::ImportError),
    DependencyMismatch { name: String, versions: Vec<String> },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use ResolveError::*;
        match self {
            Import(err) => write!(f, "{}", err),
            DependencyMismatch { name, versions } => write!(
                f,
                "Could not reconcile versions for `{}`: ({})",
                name,
                versions.join(" ")
            ),
        }
    }
}
