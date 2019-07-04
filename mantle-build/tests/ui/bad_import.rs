#[derive(mantle::Service)]
pub struct Service;

impl Service {
    pub fn new(_ctx: &mantle::Context) -> Self {
        Self
    }

    pub fn bad_import(&mut self, _ctx: &mantle::Context, arg: serde_cbor::Value) {}
}

fn main() {
    mantle::service!(Service);
}
