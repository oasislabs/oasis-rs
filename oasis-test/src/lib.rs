mod ext;

pub use ext::create_account;

#[macro_export]
macro_rules! init {
    () => {
        use oasis_std::prelude::*;
    };
}
