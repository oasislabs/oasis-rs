oasis_std::contract! {

pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Self {
        Self(42)
    }
}

}

fn main() {}
