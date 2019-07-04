use mantle::{default, Context, Service};

#[derive(Service, Default)]
pub struct MyService {
    field: Vec<String>,
}

impl MyService {
    pub fn new(_ctx: &Context) -> Self {
        Default::default()
    }

    #[default]
    pub fn default1(&mut self, _ctx: &Context) -> Result<(), ()> {
        self.field.push("default1".to_string());
        Ok(())
    }

    #[default]
    pub fn default2(&self, _ctx: &Context) -> &[String] {
        self.field.as_slice()
    }
}

fn main() {
    mantle::service!(MyService);
}
