use crate::Interface;

use super::{ImporterBackend, ImporterError};

pub struct FileImporter {
    pub path: std::path::PathBuf,
}

impl ImporterBackend for FileImporter {
    fn import(&self, name: &str) -> Result<Interface, ImporterError> {
        self.import_all()?
            .pop()
            .ok_or_else(|| ImporterError::NoImport(name.to_string()))
    }

    fn import_all(&self) -> Result<Vec<Interface>, ImporterError> {
        let fbuf = std::fs::read(&self.path).map_err(|err| {
            ImporterError::Fail(failure::format_err!("{}: {}", err, self.path.display()))
        })?;
        let iface_bytes = if self.path.extension() == Some(std::ffi::OsStr::new("wasm"))
            && &fbuf[..4] == b"\0asm"
        {
            match walrus::Module::from_buffer(&fbuf)
                .map_err(ImporterError::Fail)?
                .customs
                .remove_raw("mantle-interface")
            {
                Some(iface_section) => iface_section.data,
                None => return Err(ImporterError::MissingInterfaceSection),
            }
        } else {
            fbuf
        };
        let iface = Interface::from_slice(&iface_bytes).map_err(ImporterError::Fail)?;
        Ok(vec![iface])
    }
}
