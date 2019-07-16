use oasis_std::Context;

#[derive(oasis_std::Service)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Self {
        Self(Default::default())
    }

    pub unsafe extern "C" fn wtf<T: std::fmt::Debug>(&self, ctx: &Context, val: T) {
        println!("val: {:?}", val);
    }
}

fn main() {
    oasis_std::service!(Counter);
}
