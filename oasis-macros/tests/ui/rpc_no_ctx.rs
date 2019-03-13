oasis_std::contract! {

#[derive(Contract, Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: Context) -> Self {
        Default::default()
    }

    pub fn incr(&mut self, amount: u32) {
        self.0 += amount;
    }
}

}

fn main() {}
