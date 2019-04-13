use crate::types::*;

oasis_macros::test_client!();

mod test_ext {
    extern "C" {
        pub fn create_account(value_bytes: *const u8) -> *const u8;
        pub fn is_testing() -> bool;
        pub fn register_exports(
            addr: *const u8,
            export_names: *const *const i8,
            export_fns: *const extern "C" fn(),
            num_exports: u32,
        );
    }
}

mod mock_test_ext {
    #[no_mangle]
    #[linkage = "weak"]
    extern "C" fn create_account(_value_bytes: *const u8) -> *const u8 {
        std::ptr::null()
    }

    #[no_mangle]
    #[linkage = "weak"]
    extern "C" fn is_testing() -> bool {
        false
    }

    #[no_mangle]
    #[linkage = "weak"]
    extern "C" fn register_exports(
        _addr: *const u8,
        _export_names: *const *const i8,
        _export_fns: *const extern "C" fn(),
        _num_exports: u32,
    ) {
    }
}

pub fn call_with<T, F: FnOnce() -> T>(
    addr: &Address,
    sender: Option<&Address>,
    value: Option<&U256>,
    input: &[u8],
    gas: &U256,
    call_fn: F,
) -> T {
    match sender {
        Some(sender) => push_sender(sender),
        None => push_current_address_as_sender(),
    };
    push_address(addr);
    push_value(value.unwrap_or(&U256::zero()));
    push_input(input);
    push_gas(gas);
    let ret = call_fn();
    pop_gas();
    pop_input();
    pop_value();
    pop_address();
    pop_sender();
    ret
}

pub fn create_account<V: Into<U256>>(balance: V) -> Address {
    Address::from_raw(unsafe { test_ext::create_account(balance.into().as_ptr()) })
}

pub fn is_testing() -> bool {
    unsafe { test_ext::is_testing() }
}

pub fn register_exports(addr: Address, exports: &[(String, extern "C" fn())]) {
    let (export_names, export_fns): (Vec<std::ffi::CString>, Vec<extern "C" fn()>) = exports
        .into_iter()
        .map(|(name, func)| (std::ffi::CString::new(name.to_string()).unwrap(), func))
        .unzip();
    unsafe {
        test_ext::register_exports(
            addr.as_ptr(),
            export_names
                .iter()
                .map(|name| name.as_ptr())
                .collect::<Vec<*const i8>>()
                .as_ptr(),
            export_fns.as_ptr(),
            export_names.len() as u32,
        );
    }
}
