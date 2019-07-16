#[derive(oasis_std::Service)]
pub struct Service;

impl Service {
    pub fn new(_ctx: &oasis_std::Context) -> Self {
        Self
    }

    pub fn bad_import(&mut self, _ctx: &oasis_std::Context, arg: serde_cbor::Value) {}
}

fn main() {
    oasis_std::service!(Service);
}
