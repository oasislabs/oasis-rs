use mantle::{Context, Service};
use serde::{Deserialize, Serialize};

#[derive(Service)]
pub struct ServiceB {
    seed: Number,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Number(pub u8);

impl ServiceB {
    pub fn new(_ctx: &Context, seed: Number) -> Self {
        Self { seed }
    }

    pub fn random(&self, _ctx: &Context, count: Number) -> Vec<Number> {
        vec![Number(4); count.0 as usize]
    }
}

fn main() {
    mantle::service!(ServiceB);
}
