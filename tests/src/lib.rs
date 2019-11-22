#![feature(box_syntax)]

use oasis_std::abi::*;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct NonXccType;

#[cfg(not(target_os = "wasi"))]
pub mod mock_gateway;

#[cfg(all(not(target_os = "wasi"), test))]
mod tests;
