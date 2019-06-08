use mantle::Context;

#[derive(mantle::Service)]
pub struct NonPOD(*const u8);

impl NonPOD {
    pub fn new(ctx: &Context) -> Result<Self, ()> {
        Ok(Self(std::ptr::null()))
    }
}

fn main() {
    mantle::service!(NonPOD);
}
