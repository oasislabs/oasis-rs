#![allow(unused)]

#[macro_use]
extern crate serde;

use crate::types;
use mantle::{import, Context, Service};

// import! {
// }
// import!("file:///../../target/wasm32-wasi/debug/types.wasm" into import_1); // generates `mod import_1`

// import!(Import2, Import3 from "git://github.com/owner/repo" into import_1);

#[derive(Service)]
pub struct ImportingService {}

impl ImportingService {
    pub fn new(ctx: &Context) -> Result<Self, ()> {
        unimplemented!()
    }
}

fn main() {
    mantle::service!(ImportingService);
}
