use mantle::{Address, Context, Service};

#[derive(Service)]
pub struct ServiceA;

impl ServiceA {
    pub fn new(_ctx: &Context) -> Self {
        Self
    }

    pub fn call_b(&self, _ctx: &Context, b_addr: Address) -> Result<u32, ()> {
        let b = xcc_b::ServiceBClient::at(b_addr);
        Ok(b.random(&Context::default()).unwrap())
    }
}

fn main() {
    mantle::service!(ServiceA);
}
