#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract)]
    pub struct Counter(u32);

    #[derive(Contract)]
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
