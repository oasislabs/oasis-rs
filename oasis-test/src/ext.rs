use std::{cell::RefCell, collections::HashMap};

use oasis_std::types::*;

oasis_macros::test_host!();

#[no_mangle]
extern "C" fn is_testing() -> bool {
    true
}

thread_local! {
    static ACCOUNTS: RefCell<HashMap<Address, AccountState>> = RefCell::new(HashMap::new());
    static EXPORTS: RefCell<HashMap<Address, HashMap<String, extern "C" fn()>>> =
        RefCell::new(HashMap::new());
}

#[derive(Debug)]
struct AccountState {
    balance: U256,
    storage: HashMap<H256, Vec<u8>>,
}

impl AccountState {
    pub fn new_with_balance<V: Into<U256>>(balance: V) -> Self {
        Self {
            balance: balance.into(),
            storage: HashMap::new(),
        }
    }
}

fn cur_addr() -> Address {
    ADDRESS.with(|addr| addr.borrow().last().copied().unwrap())
}

fn with_cur_state<T, F: FnOnce(&AccountState) -> T>(f: F) -> T {
    ACCOUNTS.with(|accts| f(accts.borrow().get(&cur_addr()).unwrap()))
}

fn with_cur_state_mut<T, F: FnOnce(&mut AccountState) -> T>(f: F) -> T {
    ACCOUNTS.with(|accts| f(accts.borrow_mut().get_mut(&cur_addr()).unwrap()))
}

fn invoke_export<S: AsRef<str>>(addr: Address, name: S) {
    EXPORTS.with(|exports| {
        exports
            .borrow()
            .get(&addr)
            .and_then(|contract_exports| contract_exports.get(name.as_ref()))
            .unwrap()()
    });
}

#[no_mangle]
fn gasleft() -> U256 {
    U256::zero() // TODO (#14)
}

#[no_mangle]
pub fn get_bytes(key: *const u8, result: *mut u8) {
    with_cur_state(|state| {
        if let Some(val) = state.storage.get(&H256::from_raw(key)) {
            unsafe { result.copy_from_nonoverlapping(val.as_ptr(), val.len()) };
        }
    })
}

#[no_mangle]
pub fn get_bytes_len(key: *const u8) -> u64 {
    with_cur_state(|state| {
        if let Some(val) = state.storage.get(&H256::from_raw(key)) {
            val.len() as u64
        } else {
            0
        }
    })
}

#[no_mangle]
pub fn set_bytes(key: *const u8, bytes: *const u8, bytes_len: u64) {
    with_cur_state_mut(|state| {
        state.storage.insert(H256::from_raw(key), unsafe {
            std::slice::from_raw_parts(bytes, bytes_len as usize).to_vec()
        });
    });
}

#[no_mangle]
pub fn ccall(
    _gas: *const u8, // TODO (#14)
    address_ptr: *const u8,
    value_ptr: *const u8,
    _input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    let value = U256::from_raw(value_ptr);
    let sender = SENDER.with(|sender| *sender.borrow().last().unwrap());
    if ACCOUNTS.with(|accounts| value > accounts.borrow().get(&sender).unwrap().balance) {
        return 1;
    }
    if input_len > 0 {
        invoke_export(cur_addr(), "call");
    }
    ACCOUNTS.with(|accounts| {
        let mut accounts = accounts.borrow_mut();
        let recipient = Address::from_raw(address_ptr);
        let value = U256::from_raw(value_ptr);
        accounts.get_mut(&sender).unwrap().balance -= value;
        accounts.get_mut(&recipient).unwrap().balance += value;
    });
    0
}

#[no_mangle]
pub fn ret(ptr: *const u8, len: u32) {
    pp_receivers::push_return(ptr, len as usize);
}

#[no_mangle]
pub fn balance(address: *const u8, dest: *mut u8) {
    let addr = Address::from_raw(address);
    ACCOUNTS.with(|accounts| {
        accounts
            .borrow()
            .get(&addr)
            .unwrap()
            .balance
            .to_big_endian(unsafe { std::slice::from_raw_parts_mut(dest, 32) });
    });
}

pub fn create_account<V: Into<U256>>(balance: V) -> Address {
    ACCOUNTS.with(|accounts| {
        let mut accounts = accounts.borrow_mut();
        let addr = Address::from(accounts.len());
        accounts.insert(addr, AccountState::new_with_balance(balance));
        addr
    })
}

#[no_mangle]
pub fn create(
    endowment: *const u8,
    _code: *const u8,
    _code_len: *const u8,
    ret_addr: *mut u8,
) -> i32 {
    let addr = create_account(U256::from_raw(endowment));
    unsafe { ret_addr.copy_from_nonoverlapping(addr.as_ptr(), 20) };
    0
}

#[no_mangle]
extern "C" fn register_exports(
    addr: *const u8,
    export_names: *const *const i8,
    export_fns: *const extern "C" fn(),
    num_exports: u32,
) {
    let addr = Address::from_raw(addr);
    let export_names: Vec<String> = unsafe {
        std::slice::from_raw_parts(export_names, num_exports as usize)
            .into_iter()
            .map(|ptr| std::ffi::CStr::from_ptr(*ptr))
            .map(|cstr| cstr.to_str().unwrap().to_string())
            .collect()
    };
    let export_fns: Vec<extern "C" fn()> = unsafe {
        std::slice::from_raw_parts(export_fns, num_exports as usize)
            .into_iter()
            .map(|func| func.to_owned())
            .collect()
    };
    let addr_exports: HashMap<String, extern "C" fn()> =
        export_names.into_iter().zip(export_fns).collect();
    EXPORTS.with(|exports| exports.borrow_mut().insert(addr, addr_exports));
}
