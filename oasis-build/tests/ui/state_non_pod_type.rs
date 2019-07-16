use oasis_std::Context;

#[derive(oasis_std::Service)]
pub struct NonPOD(*const u8);

impl NonPOD {
    pub fn new(ctx: &Context) -> Result<Self, ()> {
        Ok(Self(std::ptr::null()))
    }
}

fn main() {
    oasis_std::service!(NonPOD);
}
