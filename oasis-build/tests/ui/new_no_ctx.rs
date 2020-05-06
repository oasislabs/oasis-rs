use oasis_std::Context;

#[derive(oasis_std::Service, Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: Context) -> Result<Self, String> {
        if true {
            Err("error".to_string())
        } else {
            Ok(Default::default())
        }
    }
}

fn main() {
    oasis_std::service!(Counter);
}
