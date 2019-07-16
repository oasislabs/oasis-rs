use oasis_std::{Context, Service};

#[derive(Service)]
pub struct MyService;

impl MyService {
    pub fn new(_ctx: &Context) -> Self {
        Self
    }

    #[oasis_std::default]
    pub fn bad_default(&mut self, _ctx: &Context, arg: u8) {}
}

fn main() {
    oasis_std::service!(MyService);
}
