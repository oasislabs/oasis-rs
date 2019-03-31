#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract, Default)]
    pub struct Counter(u32);

    impl Counter {
        pub fn new(ctx: &Context, start_count: u32) -> Self {
            Self(start_count)
        }
    }
}

fn main() {}
