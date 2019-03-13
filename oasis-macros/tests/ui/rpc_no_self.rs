oasis_std::contract! {

#[derive(Contract, Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: Context) -> Self {
        Default::default()
    }

    pub fn incr(self, ctx: Context) {
        self.0 += 1;
    }
}

}

fn main() {}
