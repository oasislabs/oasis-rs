fn main() {
    if cfg!(not(any(feature = "deploy", feature = "test"))) {
        oasis_std::build_contract().unwrap();
    }
}
