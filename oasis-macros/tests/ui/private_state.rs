#![feature(proc_macro_hygiene)]
#[oasis_std::service]
mod service {
    #[derive(Service)]
    struct Counter(u32);

    impl Counter {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Self(42))
        }
    }
}

fn main() {}
