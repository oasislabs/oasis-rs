oasis_std::contract! {

#[derive(Contract)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: &Context) -> Self {
        Self(42)
    }

    pub unsafe extern "C" fn wtf<T: std::fmt::Debug>(&self, ctx: &Context, val: T) {
        println!("val: {:?}", val);
    }
}

}

fn main() {}
