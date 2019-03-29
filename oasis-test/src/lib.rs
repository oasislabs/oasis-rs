pub mod ext;

pub use ext::{
    create_account, pop_address, pop_context, pop_input, push_address, push_context, push_input,
};

#[macro_export]
macro_rules! init {
    () => {};
}
