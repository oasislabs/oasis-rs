#![feature(proc_macro_hygiene)]
#[oasis_std::service]
mod service {
    #[derive(Service)]
    pub struct NonPOD(*const u8);

    impl NonPOD {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Self(std::ptr::null()))
        }
    }
}

fn main() {}
