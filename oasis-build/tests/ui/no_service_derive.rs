use oasis_std::Context;

pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Self {
        Self(42)
    }
}

fn main() {
    oasis_std::service!(Counter);
}
