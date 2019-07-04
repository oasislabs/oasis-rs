use mantle::Context;

#[derive(mantle::Service)]
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
    mantle::service!(Counter);
}
