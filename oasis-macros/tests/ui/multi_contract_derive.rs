oasis_std::contract! {

#[derive(Contract)]
pub struct Counter(u32);

#[derive(Contract)]
pub struct Counter2(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Self {
        Self(42)
    }
}

impl Counter2 {
    pub fn new(ctx: &Context) -> Self {
        Self(42)
    }
}

}

fn main() {}
