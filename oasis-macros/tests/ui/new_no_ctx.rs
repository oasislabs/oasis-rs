#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract, Default)]
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
