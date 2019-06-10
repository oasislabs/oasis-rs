use mantle::Context;

#[derive(mantle::Service)]
pub struct Counter(u32);

fn main() {
    mantle::service!(Counter);
}
