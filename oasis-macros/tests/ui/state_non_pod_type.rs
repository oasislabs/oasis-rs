#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract)]
    pub struct NonPOD(*const u8);

    impl NonPOD {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Self(std::ptr::null()))
        }
    }
}

fn main() {}
