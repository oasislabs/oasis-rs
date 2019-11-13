#![feature(box_syntax)]

#[macro_use]
extern crate serde;

mod idl;

#[cfg(feature = "import")]
pub mod import;
#[cfg(feature = "visitor")]
pub mod visitor;

use anyhow::Result;

pub use idl::*;

#[cfg(feature = "saveload")]
impl Interface {
    pub fn from_slice(sl: &[u8]) -> Result<crate::Interface> {
        use std::io::Read as _;
        let mut decoder = libflate::deflate::Decoder::new(sl);
        let mut inflated = Vec::new();
        decoder.read_to_end(&mut inflated)?;
        Ok(serde_json::from_slice(&inflated)?)
    }

    pub fn to_vec(&self) -> Result<Vec<u8>> {
        let mut encoder = libflate::deflate::Encoder::new(Vec::new());
        serde_json::to_writer(&mut encoder, self)?;
        Ok(encoder.finish().into_result()?)
    }

    pub fn to_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
