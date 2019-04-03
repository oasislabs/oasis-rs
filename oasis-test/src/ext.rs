use std::{cell::RefCell, collections::HashMap};

use oasis_std::types::*;

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

thread_local! {
    static ACCOUNTS: RefCell<HashMap<Address, AccountState>> = RefCell::new(HashMap::new());
    static EXPORTS: RefCell<HashMap<Address, HashMap<String, &'static (dyn Fn())>>> =
        RefCell::new(HashMap::new());
}

oasis_macros::test_pp_host!();

fn cur_addr() -> Address {
    ADDRESS.with(|addr| {
        let addr = addr.borrow();
        addr.last().copied().unwrap()
    })
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
    U256::zero() // TODO
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
    _gas: *const u8, // TODO
    address_ptr: *const u8,
    value_ptr: *const u8,
    _input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    let value = U256::from_raw(value_ptr);
    let sender = SENDER.with(|sender| {
        let sender = sender.borrow();
        if sender.len() > 1 {
            cur_addr()
        } else {
            sender.last().copied().unwrap()
        }
    });
    if value > with_cur_state(|state| state.balance) {
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
        accounts.insert(addr.clone(), AccountState::new_with_balance(balance));
        addr
    })
}
