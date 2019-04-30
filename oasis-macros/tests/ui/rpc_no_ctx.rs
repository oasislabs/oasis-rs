#![feature(proc_macro_hygiene)]
#[oasis_std::service]
mod service {
    #[derive(Service, Default)]
    pub struct Counter(u32);

    impl Counter {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Default::default())
        }

        pub fn incr(&mut self, ctx: Context, amount: u32) -> Result<()> {
            self.0 += amount;
            Ok(())
        }
    }
}

fn main() {}
