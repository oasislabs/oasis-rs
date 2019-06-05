#![feature(proc_macro_hygiene)]
#[mantle::service]
mod service {
    #[derive(Service, Default)]
    pub struct Counter(u32);

    impl Counter {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Default::default())
        }

        pub fn incr(self, ctx: &Context) -> Result<()> {
            self.0 += 1;
            Ok(())
        }
    }
}

fn main() {}
