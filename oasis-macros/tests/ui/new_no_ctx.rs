#![feature(proc_macro_hygiene)]
#[oasis_std::service]
mod service {
    #[derive(Service, Default)]
    pub struct Counter(u32);

    impl Counter {
        pub fn new(ctx: Context) -> Result<Self> {
            if true {
                Err(failure::format_err!("{}", Default::default()));
            } else {
                Ok(Default::default())
            }
        }
    }
}

fn main() {}
