#[derive(mantle::Service)]
pub struct Printer;

impl Printer {
    pub fn new(ctx: &mantle::Context) -> Self {
        println!("hello, world!");
        Self
    }

    pub fn print(&self, ctx: &mantle::Context) {
        print!("sender: {:?}", ctx.sender());
        eprintln!("this is k");
        dbg!("this is also k");
    }
}

fn main() {
    mantle::service!(Printer);
}
