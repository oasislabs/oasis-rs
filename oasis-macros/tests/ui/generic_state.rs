oasis_std::contract! {

#[derive(Contract, Default)]
pub struct State<T>(Option<T>);

impl<T: Default> State<T> {
    pub fn new(ctx: Context) -> Self {
        Default::default()
    }

    fn hmmm() {
        println!("hmmm");
    }
}

}

fn main() {}
