use crate::import::{ImportError, ImportLocation, ImportedService, Importer};

pub fn resolve_imports(
    deps: impl IntoIterator<Item = (String, ImportLocation)>,
    base_dir: &std::path::Path,
) -> Result<Vec<ImportedService>, ResolveError> {
    deps.into_iter()
        .map(|(name, loc)| {
            Importer::for_location(loc, base_dir)
                .and_then(|importer| importer.import(&name))
                .map_err(ResolveError::Import)
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    Import(ImportError),
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
