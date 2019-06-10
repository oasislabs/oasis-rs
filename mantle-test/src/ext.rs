use mantle_types::{Address, ExtStatusCode};

#[no_mangle]
static mantle_testing: bool = true;

#[no_mangle]
pub extern "C" fn mantle_balance(addr: *const Address, balance: *mut u64) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_code(addr: *const Address, buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_code_len(at: *const Address, len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_fetch_input(buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_input_len(len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_ret(buf: *const u8, len: u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_err(buf: *const u8, len: u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_fetch_ret(buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_ret_len(len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_fetch_err(buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_err_len(len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_transact(
    callee: *const Address,
    value: u64,
    input: *const u8,
    input_len: u32,
) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_address(addr: *mut Address) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_sender(addr: *mut Address) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_value(value: *mut u64) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_read(key: *const u8, key_len: u32, value: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_read_len(
    key: *const u8,
    key_len: u32,
    value_len: *mut u32,
) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_write(
    key: *const u8,
    key_len: u32,
    value: *const u8,
    value_len: u32,
) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn mantle_emit(
    topics: *const *const u8,
    topic_lens: *const u32,
    num_topics: u32,
    data: *const u8,
    data_len: u32,
) -> ExtStatusCode {
    ExtStatusCode::Success
}
