#![feature(proc_macro_hygiene)]
#[oasis_std::service]
mod service {
    #[derive(Service)]
    pub struct Counter(u32);

    impl Counter {
        pub fn new(ctx: &Context) -> Result<Self> {
            Ok(Self(42))
        }

        pub unsafe extern "C" fn wtf<T: std::fmt::Debug>(
            &self,
            ctx: &Context,
            val: T,
        ) -> Result<()> {
            println!("val: {:?}", val);
            Ok(())
        }
    }
}

fn main() {}
