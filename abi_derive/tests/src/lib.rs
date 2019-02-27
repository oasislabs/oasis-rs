#![cfg(test)]

#[macro_use]
extern crate lazy_static;

mod arrays;
mod contract;
mod erc20;
mod general;
mod multiple_return;
mod payable;
mod strings;
mod trivia;

use std::{cell::Cell, sync::Mutex};

lazy_static! {
    static ref VALUE: Mutex<Cell<u64>> = Mutex::new(Cell::new(0));
    static ref CALL_PAYLOAD: Mutex<Option<Vec<u8>>> = Mutex::new(None);
}

/// mock externs

#[no_mangle]
pub unsafe fn value(dest: *mut u8) {
    let val_bytes = dbg!(VALUE.lock().unwrap().replace(0).to_be_bytes());
    dest.copy_from_nonoverlapping(val_bytes.as_ptr(), val_bytes.len());
}

fn set_value(val: u64) {
    VALUE.lock().unwrap().set(val);
}

#[no_mangle]
pub unsafe fn ccall(
    _gas: i64,
    _address: *const u8,
    _val_ptr: *const u8,
    input_ptr: *const u8,
    input_len: u32,
    _result_ptr: *mut u8,
    _result_len: u32,
) -> i32 {
    let payload = std::slice::from_raw_parts(input_ptr, input_len as usize);
    CALL_PAYLOAD.lock().unwrap().replace(payload.to_vec());
    0
}

fn get_call_payload() -> Vec<u8> {
    CALL_PAYLOAD.lock().unwrap().take().unwrap()
}
