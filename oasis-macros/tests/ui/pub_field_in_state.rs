oasis_std::contract! {

#[derive(Contract, Default)]
pub struct Counter {
    pub count: u32
}

impl Counter {
    pub fn new(ctx: &Context) -> Self {
        Default::default()
    }
}

}

fn main() {}
