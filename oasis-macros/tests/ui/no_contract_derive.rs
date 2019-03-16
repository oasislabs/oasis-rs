oasis_std::contract! {

pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Result<Self> {
        Ok(Self(42))
    }
}

}

fn main() {}
