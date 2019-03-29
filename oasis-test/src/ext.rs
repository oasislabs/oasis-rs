use std::{cell::RefCell, collections::HashMap};

use oasis_std::{ext, types::*};

thread_local! {
    // vec index is account
    static ACCOUNTS: RefCell<Vec<U256>> = RefCell::new(Vec::new());
    static STATE: RefCell<Vec<HashMap<H256, Vec<u8>>>> = RefCell::new(Vec::new());

    // vec index is call stack depth
    static ADDRESS: RefCell<Vec<Address>> = RefCell::new(Vec::new());
    static INPUT: RefCell<Vec<Vec<u8>>> = RefCell::new(Vec::new());
    static RET: RefCell<Vec<Vec<u8>>> = RefCell::new(Vec::new());
    static SENDER: RefCell<Vec<Address>> = RefCell::new(Vec::new());
    static VALUE: RefCell<Vec<U256>> = RefCell::new(Vec::new());
    static GAS: RefCell<Vec<u64>> = RefCell::new(Vec::new());
}

fn addr_idx() -> usize {
    ext::address().to_low_u64_be() as usize
}

#[no_mangle]
pub fn sender(dest: *mut u8) {
    SENDER.with(|sender| unsafe {
        dest.copy_from_nonoverlapping(sender.borrow().last().unwrap().as_ptr(), 20)
    });
}

#[no_mangle]
pub fn address(dest: *mut u8) {
    ADDRESS.with(|address| unsafe {
        dest.copy_from_nonoverlapping(address.borrow().last().unwrap().as_ptr(), 20)
    });
}

#[no_mangle]
pub fn gasleft() -> u64 {
    0 // TODO
}

#[no_mangle]
pub fn get_bytes(key: *const u8, result: *mut u8) {
    STATE.with(|state| {
        if let Some(val) = state.borrow()[addr_idx()].get(&H256::from_raw(key)) {
            unsafe { result.copy_from_nonoverlapping(val.as_ptr(), val.len()) };
        }
    });
}

#[no_mangle]
pub fn get_bytes_len(key: *const u8) -> u64 {
    STATE.with(|state| {
        if let Some(val) = state.borrow()[addr_idx()].get(&H256::from_raw(key)) {
            val.len() as u64
        } else {
            0
        }
    })
}

#[no_mangle]
pub fn set_bytes(key: *const u8, bytes: *const u8, bytes_len: u64) {
    STATE.with(|state| {
        state.borrow_mut()[addr_idx()].insert(H256::from_raw(key), unsafe {
            std::slice::from_raw_parts(bytes, bytes_len as usize).to_vec()
        })
    });
}

#[no_mangle]
pub fn input_length() -> u32 {
    INPUT.with(|inp| inp.borrow().last().unwrap().len() as u32)
}

#[no_mangle]
pub fn fetch_input(dest: *mut u8) {
    INPUT.with(|inp| {
        let inps = inp.borrow();
        let inp = inps.last().unwrap();
        unsafe { dest.copy_from_nonoverlapping(inp.as_ptr(), inp.len()) };
    });
}

extern "C" {
    fn call();
}

#[no_mangle]
pub fn ccall(
    _gas: u64, // TODO
    address_ptr: *const u8,
    value_ptr: *const u8,
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    let value = U256::from_raw(value_ptr);
    let sender = SENDER.with(|sender| {
        if sender.borrow().len() > 1 {
            ext::address()
        } else {
            ext::sender()
        }
    });
    if value > ext::balance(&sender) {
        return 1;
    }
    push_input(unsafe { std::slice::from_raw_parts(input_ptr, input_len as usize) }.to_vec());
    if input_len > 0 {
        unsafe { call() };
    }
    ACCOUNTS.with(|accounts| {
        let mut accounts = accounts.borrow_mut();
        let recipient = Address::from_raw(address_ptr);
        let value = U256::from_raw(value_ptr);
        accounts[sender.to_low_u64_be() as usize] -= value;
        accounts[recipient.to_low_u64_be() as usize] += value;
    });
    pop_input();
    0
}

#[no_mangle]
pub fn ret(ptr: *const u8, len: u32) {
    RET.with(|ret| {
        ret.borrow_mut()
            .push(unsafe { std::slice::from_raw_parts(ptr, len as usize).to_vec() })
    });
}

#[no_mangle]
pub fn return_length() -> u32 {
    RET.with(|ret| ret.borrow().last().unwrap().len() as u32)
}

#[no_mangle]
pub fn fetch_return(dest: *mut u8) {
    RET.with(|ret| {
        let rets = ret.borrow();
        let ret = rets.last().unwrap();
        unsafe { dest.copy_from_nonoverlapping(ret.as_ptr(), ret.len()) };
    });
}

#[no_mangle]
pub fn value(dest: *mut u8) {
    VALUE.with(|val| {
        val.borrow()
            .last()
            .unwrap()
            .to_big_endian(unsafe { &mut std::slice::from_raw_parts_mut(dest, 32) })
    });
}

#[no_mangle]
pub fn balance(address: *const u8, dest: *mut u8) {
    let addr = Address::from_raw(address);
    ACCOUNTS.with(|accounts| {
        let balance = accounts.borrow()[addr.to_low_u64_be() as usize];
        balance.to_big_endian(unsafe { std::slice::from_raw_parts_mut(dest, 32) });
    });
}

pub fn create_account<V: Into<U256>>(balance: V) -> Address {
    STATE.with(|state| state.borrow_mut().push(HashMap::new()));
    ACCOUNTS.with(|accounts| {
        let mut accounts = accounts.borrow_mut();
        let i = accounts.len();
        accounts.push(balance.into());
        i.into()
    })
}

pub fn push_address(address: Address) {
    ADDRESS.with(|addr| addr.borrow_mut().push(address));
}

pub fn pop_address() {
    ADDRESS.with(|addr| addr.borrow_mut().pop());
}

pub fn push_input(input: Vec<u8>) {
    INPUT.with(|inp| inp.borrow_mut().push(input));
}

pub fn pop_input() {
    INPUT.with(|inp| inp.borrow_mut().pop());
}

fn push_sender(sender: Address) {
    SENDER.with(|s| s.borrow_mut().push(sender));
}

fn pop_sender() {
    SENDER.with(|s| s.borrow_mut().pop());
}

fn push_value(value: U256) {
    VALUE.with(|val| val.borrow_mut().push(value));
}

fn pop_value() {
    VALUE.with(|val| val.borrow_mut().pop());
}

pub fn push_context(ctx: &oasis_std::exe::Context) {
    push_sender(ctx.sender.unwrap());
    push_value(ctx.value.unwrap_or_default());
}

pub fn pop_context() {
    pop_value();
    pop_sender();
}
