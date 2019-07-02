use crate::{import::ImportedService, Interface};

use super::{ImporterBackend, ImporterError};

pub struct FileImporter {
    pub path: std::path::PathBuf,
}

impl ImporterBackend for FileImporter {
    fn import(&self, name: &str) -> Result<ImportedService, ImporterError> {
        self.import_all()?
            .pop()
            .ok_or_else(|| ImporterError::NoImport(name.to_string()))
    }

    fn import_all(&self) -> Result<Vec<ImportedService>, ImporterError> {
        let bytecode = std::fs::read(&self.path).map_err(|err| {
            ImporterError::Fail(failure::format_err!("{}: {}", err, self.path.display()))
        })?;
        let interface_bytes = match walrus::Module::from_buffer(&bytecode)
            .map_err(ImporterError::Fail)?
            .customs
            .remove_raw("mantle-interface")
        {
            Some(iface_section) => iface_section.data,
            None => return Err(ImporterError::MissingInterfaceSection),
        };
        let interface = Interface::from_slice(&interface_bytes).map_err(ImporterError::Fail)?;
        Ok(vec![ImportedService {
            bytecode,
            interface,
        }])
    }
}
