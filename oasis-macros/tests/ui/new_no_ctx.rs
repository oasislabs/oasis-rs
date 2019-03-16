oasis_std::contract! {

#[derive(Contract)]
#[derive(Default)]
pub struct Counter(u32);

impl Counter {
    pub fn new(ctx: Context) -> Result<Self> {
        if true {
            Err(failure::format_err!("{}", Default::default()));
        } else {
            Ok(Default::default())
        }
    }
}

}

fn main() {}
