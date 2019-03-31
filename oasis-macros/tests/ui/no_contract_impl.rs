#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract)]
    pub struct Counter(u32);
}

fn main() {}
