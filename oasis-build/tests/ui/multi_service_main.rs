use oasis_std::Context;

#[derive(oasis_std::Service)]
pub struct Counter(u32);

#[derive(oasis_std::Service)]
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
    oasis_std::service!(Counter);
    oasis_std::service!(Counter2);
}
