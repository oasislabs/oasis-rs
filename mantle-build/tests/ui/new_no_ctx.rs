use mantle::Context;

#[derive(mantle::Service, Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: Context) -> Result<Self, String> {
        if true {
            Err(format!("{}", Default::default()))
        } else {
            Ok(Default::default())
        }
    }
}

fn main() {
    mantle::service!(Counter);
}
