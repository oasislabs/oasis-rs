oasis_std::contract! {

#[derive(Contract)]
#[derive(Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Self {
        Default::default()
    }
}

}

fn main() {}
