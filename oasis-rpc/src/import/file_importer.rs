use crate::{import::ImportedService, Interface};

use super::{ImportError, ImporterBackend};

pub struct FileImporter {
    pub path: std::path::PathBuf,
}

impl ImporterBackend for FileImporter {
    fn import(&self, name: &str) -> Result<ImportedService, ImportError> {
        self.import_all()?
            .pop()
            .ok_or_else(|| ImportError::NoImport(name.to_string()))
    }

    fn import_all(&self) -> Result<Vec<ImportedService>, ImportError> {
        let bytecode = std::fs::read(&self.path)
            .map_err(|err| ImportError::Io(self.path.display().to_string(), err))?;
        let interface = Interface::from_wasm_bytecode(&bytecode).map_err(ImportError::Importer)?;
        Ok(vec![ImportedService {
            bytecode,
            interface,
        }])
    }
}
