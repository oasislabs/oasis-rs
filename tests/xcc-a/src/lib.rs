#![feature(proc_macro_hygiene)]
#[mantle::service]
mod service {
    #[derive(Service)]
    pub struct ServiceA {
        b_addr: Address,
    }

    impl ServiceA {
        pub fn new(_ctx: &Context, b_addr: Address) -> Result<Self> {
            Ok(Self { b_addr })
        }

        pub fn do_the_thing(&self, ctx: &Context) -> Result<u64> {
            let b_ctx = Context::default().with_value(ctx.value());
            xcc_b::ServiceB::at(self.b_addr).record_value(&b_ctx)
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xcc() {
        // 1. create user with initial `val`
        // 2. transfer all value to `ServiceA`
        // 3. create `ServiceB` which records the amount of value passed through it
        // 4. transfer `val - 1` to `ServiceB`
        // 5. transfer `1` to `ServiceB`

        let val = 0x0A515u64;

        let user = mantle_test::create_account(val);
        let ctx = Context::default().with_sender(user);

        let b = xcc_b::ServiceB::new(&ctx).unwrap();
        let a = ServiceA::new(&ctx.with_value(val), b.address()).unwrap();

        assert_eq!(a.do_the_thing(&ctx.with_value(val - 1)).unwrap(), val - 1);
        assert_eq!(b.total_value(&ctx).unwrap(), val - 1);

        assert_eq!(a.do_the_thing(&ctx.with_value(1)).unwrap(), 1u32);
        assert_eq!(b.total_value(&ctx).unwrap(), val);
    }
}
