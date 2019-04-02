use crate::{exe::Context, types::*};

oasis_macros::define_test_pp!({
    address: &Address,
    input: &[u8],
    sender: &Address,
    value: &U256,
});

mod test_ext {
    extern "C" {
        pub fn create_account(value_bytes: *const u8) -> *const u8;
    }
}

mod mock_test_ext {
    #[no_mangle]
    #[linkage = "extern_weak"]
    extern "C" fn create_account(_value_bytes: *const u8) -> *const u8 {
        std::ptr::null()
    }
}

pub fn call_with<T>(addr: &Address, ctx: &Context, input: &[u8], call_fn: &(dyn Fn() -> T)) -> T {
    push_address(addr);
    push_sender(ctx.sender.as_ref().unwrap());
    push_value(ctx.value.as_ref().unwrap_or(&U256::zero()));
    push_input(input);
    let ret = call_fn();
    pop_input();
    pop_value();
    pop_sender();
    pop_address();
    ret
}

pub fn create_account<V: Into<U256>>(balance: V) -> Address {
    Address::from_raw(unsafe { test_ext::create_account(balance.into().as_ptr()) })
}
