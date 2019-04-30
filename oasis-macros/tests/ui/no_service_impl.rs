#![feature(proc_macro_hygiene)]
#[oasis_std::service]
mod service {
    #[derive(Service)]
    pub struct Counter(u32);
}

fn main() {}
