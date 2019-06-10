use mantle::Context;

#[derive(mantle::Service)]
pub struct Counter(u32);

#[derive(mantle::Service)]
pub struct Counter2(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Result<Self, ()> {
        Ok(Self(42))
    }
}

impl Counter2 {
    pub fn new(ctx: &Context) -> Result<Self, ()> {
        Ok(Self(42))
    }
}

fn main() {
    mantle::service!(Counter);
    mantle::service!(Counter2);
}
