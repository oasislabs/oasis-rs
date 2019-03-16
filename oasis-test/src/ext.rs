use std::{cell::RefCell, collections::HashMap};

use oasis_std::types::*;

fn key_from_ptr(key: *const u8) -> H256 {
    H256::from_slice(unsafe { std::slice::from_raw_parts(key, 32) })
}

thread_local! {
    static STATE: RefCell<HashMap<H256, Vec<u8>>> = RefCell::new(HashMap::new());
    static INPUT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    static RET: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    static SENDER: RefCell<Address> = RefCell::new(Address::zero());
}

#[no_mangle]
pub fn sender(dest: *mut u8) {
    SENDER.with(|sender| {
        unsafe { dest.copy_from_nonoverlapping(sender.borrow().as_ptr(), 20) };
    });
}

#[no_mangle]
pub fn get_bytes(key: *const u8, result: *mut u8) {
    STATE.with(|state| {
        if let Some(val) = state.borrow().get(&key_from_ptr(key)) {
            unsafe { result.copy_from_nonoverlapping(val.as_ptr(), val.len()) };
        }
    });
}

#[no_mangle]
pub fn get_bytes_len(key: *const u8) -> u64 {
    STATE.with(|state| {
        if let Some(val) = state.borrow().get(&key_from_ptr(key)) {
            val.len() as u64
        } else {
            0
        }
    })
}

#[no_mangle]
pub fn set_bytes(key: *const u8, bytes: *const u8, bytes_len: u64) {
    STATE.with(|state| {
        state.borrow_mut().insert(key_from_ptr(key), unsafe {
            std::slice::from_raw_parts(bytes, bytes_len as usize).to_vec()
        });
    });
}

#[no_mangle]
pub fn input_length() -> u32 {
    INPUT.with(|inp| inp.borrow().len() as u32)
}

#[no_mangle]
pub fn fetch_input(dest: *mut u8) {
    INPUT.with(|inp| {
        let inp = inp.borrow();
        unsafe { dest.copy_from_nonoverlapping(inp.as_ptr(), inp.len()) };
    });
}

extern "C" {
    fn call();
}

#[no_mangle]
pub fn ccall(
    // TODO
    _gas: u64,
    _address: *const u8,
    _val_ptr: *const u8,
    input_ptr: *const u8,
    input_len: u32,
    _result_ptr: *mut u8,
    _result_len: u32,
) -> u32 {
    set_input(unsafe { std::slice::from_raw_parts(input_ptr, input_len as usize) }.to_vec());
    unsafe { call() };
    0
}

#[no_mangle]
pub fn ret(ptr: *const u8, len: u32) {
    RET.with(|ret| {
        *ret.borrow_mut() = unsafe { std::slice::from_raw_parts(ptr, len as usize).to_vec() };
    });
}

#[no_mangle]
pub fn return_length() -> u32 {
    RET.with(|ret| ret.borrow().len() as u32)
}

#[no_mangle]
pub fn fetch_return(dest: *mut u8) {
    RET.with(|ret| {
        let ret = ret.borrow();
        unsafe { dest.copy_from_nonoverlapping(ret.as_ptr(), ret.len()) };
    });
}

pub(crate) fn set_sender(sender: Address) {
    SENDER.with(|s| {
        *s.borrow_mut() = sender;
    });
}

pub fn set_input(input: Vec<u8>) {
    INPUT.with(|inp| {
        *inp.borrow_mut() = input;
    });
}
