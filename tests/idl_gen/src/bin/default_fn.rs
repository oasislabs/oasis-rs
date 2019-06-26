#![allow(unused)]

#[macro_use]
extern crate serde;

use mantle::{Context, Service};

#[derive(Service)]
pub struct DefaultFnService {}

impl DefaultFnService {
    pub fn new(ctx: &Context) -> Result<Self, ()> {
        unimplemented!()
    }

    pub fn default(&mut self, ctx: &Context) -> Result<(), ()> {
        unimplemented!()
    }
}

fn main() {
    mantle::service!(DefaultFnService);
}

#[test]
fn test_default_fn() {
    let idl_json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/service/DefaultFnService.json"
    ))
    .unwrap();

    let actual: serde_json::Value = serde_json::from_str(&idl_json).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/res/DefaultFnService.json"
    )))
    .unwrap();

    assert_eq!(actual, expected);
}
