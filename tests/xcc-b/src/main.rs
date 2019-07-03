use mantle::{Context, Service};

#[derive(Service)]
pub struct ServiceB;

pub struct ImportMe(u8, u16, u32, u64);

impl ServiceB {
    pub fn new(_ctx: &Context) -> Self {
        Self
    }

    pub fn random(&self, _ctx: &Context) -> u32 {
        4
    }
}

fn main() {
    mantle::service!(ServiceB);
}
