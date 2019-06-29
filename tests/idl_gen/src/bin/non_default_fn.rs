#![allow(unused)]

#[macro_use]
extern crate serde;

use mantle::{Context, Service};

#[derive(Service)]
pub struct NonDefaultFnService {}

impl DefaultFnService {
    pub fn new(ctx: &Context) -> Result<Self, String> {
        unimplemented!()
    }

    pub fn default(&mut self, ctx: &Context, extra_arg: u8) {
        unimplemented!()
    }
}

fn main() {
    mantle::service!(NonDefaultFnService);
}
