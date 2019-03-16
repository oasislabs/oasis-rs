oasis_std::contract! {

#[derive(Contract, Default)]
pub struct Counter {
    count: u32,
    max_count: u64,
}

impl Counter {
    pub(crate) fn new(ctx: &Context) -> Result<Self> {
        Ok(Default::default())
    }
}

}

fn main() {}
