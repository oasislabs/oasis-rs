#![feature(proc_macro_hygiene)]
#[mantle::service]
mod service {
    #[derive(Service)]
    pub struct Counter(u32);
}

fn main() {}
