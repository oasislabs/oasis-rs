#![allow(improper_ctypes, unused)] // ExtStatusCode is `repr(u32)` but non-exhaustive

use oasis_types::{Address, ExtStatusCode};

#[no_mangle]
static oasis_testing: bool = true;

#[no_mangle]
pub extern "C" fn oasis_balance(addr: *const Address, balance: *mut u128) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_code(addr: *const Address, buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_code_len(at: *const Address, len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_fetch_input(buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_input_len(len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_fetch_aad(buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_aad_len(len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_ret(buf: *const u8, len: u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_err(buf: *const u8, len: u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_fetch_ret(buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_ret_len(len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_fetch_err(buf: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_err_len(len: *mut u32) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_transact(
    callee: *const Address,
    value: u128,
    input: *const u8,
    input_len: u32,
) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_address(addr: *mut Address) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_sender(addr: *mut Address) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_value(value: *mut u128) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_read(key: *const u8, key_len: u32, value: *mut u8) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_read_len(
    key: *const u8,
    key_len: u32,
    value_len: *mut u32,
) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_write(
    key: *const u8,
    key_len: u32,
    value: *const u8,
    value_len: u32,
) -> ExtStatusCode {
    ExtStatusCode::Success
}

#[no_mangle]
pub extern "C" fn oasis_emit(
    topics: *const *const u8,
    topic_lens: *const u32,
    num_topics: u32,
    data: *const u8,
    data_len: u32,
) -> ExtStatusCode {
    ExtStatusCode::Success
}
