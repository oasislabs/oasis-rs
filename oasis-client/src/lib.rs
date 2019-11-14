#[macro_use]
extern crate log;

pub mod api;
pub mod gateway;
mod polling;

pub use gateway::{HttpGateway, HttpGatewayBuilder};
