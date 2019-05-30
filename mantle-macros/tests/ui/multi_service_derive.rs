#![feature(proc_macro_hygiene)]
#[mantle::service]
mod service {
    #[derive(Service)]
    pub struct Counter(u32);

    #[derive(Service)]
    pub struct Counter2(u32);

    impl Counter {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Self(42))
        }
    }

    impl Counter2 {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Self(42))
        }
    }
}

fn main() {}
