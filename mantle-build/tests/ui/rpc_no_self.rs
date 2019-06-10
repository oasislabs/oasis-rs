use mantle::Context;

#[derive(mantle::Service, Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Result<Self, ()> {
        Ok(Default::default())
    }

    pub fn incr(mut self, ctx: &Context) -> Result<(), ()> {
        self.0 += 1;
        Ok(())
    }
}

fn main() {
    mantle::service!(Counter);
}
