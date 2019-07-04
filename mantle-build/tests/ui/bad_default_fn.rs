use mantle::{Context, Service};

#[derive(Service)]
pub struct MyService;

impl MyService {
    pub fn new(_ctx: &Context) -> Self {
        Self
    }

    #[mantle::default]
    pub fn bad_default(&mut self, _ctx: &Context, arg: u8) {}
}

fn main() {
    mantle::service!(MyService);
}
