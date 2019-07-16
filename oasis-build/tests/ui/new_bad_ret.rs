use oasis_std::Context;

#[derive(oasis_std::Service, Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context, start_count: u32) -> () {
        Self(start_count);
    }
}

fn main() {
    oasis_std::service!(Counter);
}
