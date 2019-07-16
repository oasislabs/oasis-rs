#[derive(oasis_std::Service)]
pub struct Printer;

impl Printer {
    pub fn new(ctx: &oasis_std::Context) -> Self {
        println!("hello, world!");
        Self
    }

    pub fn print(&self, ctx: &oasis_std::Context) {
        print!("sender: {:?}", ctx.sender());
        eprintln!("this is k");
        dbg!("this is also k");
    }
}

fn main() {
    oasis_std::service!(Printer);
}

fn random_function() {
    println!("don't warn");
}
