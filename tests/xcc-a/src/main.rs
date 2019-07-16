use oasis_std::{Address, Context, Service};

#[derive(Service)]
pub struct ServiceA;

impl ServiceA {
    pub fn new(_ctx: &Context) -> Self {
        Self
    }

    pub fn call_b(&self, _ctx: &Context, b_addr: Address) -> Result<Vec<xcc_b::Number>, ()> {
        let b = xcc_b::ServiceBClient::at(b_addr);
        Ok(b.random(&Context::default(), xcc_b::Number(42)).unwrap())
    }
}

fn main() {
    oasis_std::service!(ServiceA);
}
