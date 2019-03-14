use oasis_std::{exe::Context, types::Address};

pub mod ext;

pub trait TestContext {
    fn set_sender(&mut self, sender: Address) -> &mut Self {
        ext::set_sender(sender);
        self
    }
}

impl TestContext for Context {}

#[macro_export]
macro_rules! init {
    () => {
        use oasis_test::TestContext;
    };
}
