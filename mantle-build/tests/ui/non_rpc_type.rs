use mantle::Context;

#[derive(mantle::Service, Default)]
pub struct Counter(usize);

impl Counter {
    pub fn new(ctx: &Context, start_count: Box<u32>) -> Result<Self, ()> {
        unimplemented!();
    }
}

fn main() {
    mantle::service!(Counter);
}
