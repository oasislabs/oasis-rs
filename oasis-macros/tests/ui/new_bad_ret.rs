oasis_std::contract! {

#[derive(Contract)]
#[derive(Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: Context, start_count: u32) -> () {
        Self(start_count);
    }
}

}

fn main() {}
