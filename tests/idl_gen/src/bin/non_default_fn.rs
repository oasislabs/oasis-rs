#![allow(unused)]

#[macro_use]
extern crate serde;

use mantle::{Context, Service};

#[derive(Service)]
pub struct NonDefaultFnService {}

impl NonDefaultFnService {
    pub fn new(ctx: &Context) -> Result<Self, String> {
        unimplemented!()
    }

    // NB: no #[default]
    pub fn default(&self, _ctx: &Context) {
        unimplemented!()
    }
}

fn main() {
    mantle::service!(NonDefaultFnService);
}
