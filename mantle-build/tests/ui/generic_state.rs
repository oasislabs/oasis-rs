use mantle::Context;

#[derive(mantle::Service, Default)]
pub struct State<T>(Option<T>);

impl<T: Default> State<T> {
    pub fn new(ctx: &Context) -> Result<Self, ()> {
        Ok(Default::default())
    }

    fn hmmm() -> Result<(), String> {
        Err(format!("hmm"))
    }
}

fn main() {
    mantle::service!(State);
}
