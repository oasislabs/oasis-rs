#![feature(proc_macro_hygiene)]
#[mantle::service]
mod service {
    #[derive(Service, Default)]
    pub struct ServiceB {
        total_value: U256,
    }

    impl ServiceB {
        pub fn new(_ctx: &Context) -> Result<Self> {
            Ok(Default::default())
        }

        /// Records the `value` passed to this service. Returns the transferred value.
        pub fn record_value(&mut self, ctx: &Context) -> Result<U256> {
            self.total_value += ctx.value();
            Ok(ctx.value())
        }

        /// Returns the total value transferred to this service (c.f. `balance`).
        pub fn total_value(&self, _ctx: &Context) -> Result<U256> {
            Ok(self.total_value)
        }
    }

}
