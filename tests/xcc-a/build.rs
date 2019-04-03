#[cfg(not(any(feature = "deploy", feature = "test")))]
fn main() {
    oasis_std::build_contract().unwrap();
}

#[cfg(any(feature = "deploy", feature = "test"))]
fn main() {}
