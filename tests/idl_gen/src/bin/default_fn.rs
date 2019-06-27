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
