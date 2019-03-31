#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    pub struct Counter(u32);

    impl Counter {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Self(42))
        }
    }
}

fn main() {}
