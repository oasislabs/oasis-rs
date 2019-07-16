use oasis_std::Context;

#[derive(oasis_std::Service, Default)]
pub struct Counter(usize);

impl Counter {
    pub fn new(ctx: &Context, start_count: Box<u32>) -> Result<Self, ()> {
        unimplemented!();
    }
}

fn main() {
    oasis_std::service!(Counter);
}
