use oasis_std::Context;

#[derive(oasis_std::Service)]
pub struct Counter(u32);

fn main() {
    oasis_std::service!(Counter);
}
