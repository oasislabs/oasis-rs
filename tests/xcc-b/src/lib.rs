#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract, Default)]
    pub struct ContractB {
        total_value: U256,
    }

    impl ContractB {
        pub fn new(_ctx: &Context) -> Result<Self> {
            Ok(Default::default())
        }

        /// Records the `value` passed to this contract. Returns the transferred value.
        pub fn record_value(&mut self, ctx: &Context) -> Result<U256> {
            self.total_value += ctx.value();
            Ok(ctx.value())
        }

        /// Returns the total value transferred to this contract (c.f. `balance`).
        pub fn total_value(&self, _ctx: &Context) -> Result<U256> {
            Ok(self.total_value)
        }
    }

}
