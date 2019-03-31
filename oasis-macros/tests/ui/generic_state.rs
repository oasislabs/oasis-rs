#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract, Default)]
    pub struct State<T>(Option<T>);

    impl<T: Default> State<T> {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Default::default())
        }

        fn hmmm() -> Result<()> {
            Err(failure::format_err!("hmm"))
        }
    }
}

fn main() {}
