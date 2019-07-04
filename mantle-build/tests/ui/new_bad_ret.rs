use mantle::Context;

#[derive(mantle::Service, Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context, start_count: u32) -> () {
        Self(start_count);
    }
}

fn main() {
    mantle::service!(Counter);
}
