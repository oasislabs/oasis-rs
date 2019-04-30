#![feature(proc_macro_hygiene)]
#[oasis_std::service]
mod service {
    #[derive(Service, Default)]
    pub struct Counter(u32);

    impl Counter {
        pub fn new(ctx: &Context, start_count: u32) -> Self {
            Self(start_count)
        }
    }
}

fn main() {}
