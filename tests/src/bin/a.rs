use oasis_std::{Address, Context, Service};

#[derive(Service)]
pub struct ServiceA;

impl ServiceA {
    pub fn new(_ctx: &Context, message: String) -> Self {
        eprintln!("{}", message);
        Self
    }

    pub fn call_b(&self, _ctx: &Context, b_addr: Address) -> Result<Vec<b::Number>, ()> {
        let b = b::ServiceBClient::new(b_addr);
        b.say_hello(
            &Context::default(),
            b::Greeting::Informal("dawg".to_string()),
        )
        .unwrap();
        b.return_ref_struct(&Context::default(), "value").unwrap();
        Ok(b.random(&Context::default(), b::Number(42)).unwrap())
    }
}

fn main() {
    oasis_std::service!(ServiceA);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_xcc() {
        let a = xcc_a::ServiceAClient::deploy();
    }
}
