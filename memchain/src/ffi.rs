use std::{borrow::Cow, cell::RefCell, collections::hash_map::Entry, ffi::CStr, rc::Rc, slice};

use oasis_types::{Address, U256};

use crate::{Account, Blockchain};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CAccount<'a> {
    address: Address,
    balance: U256,
    code: *const u8,
    code_len: u64,
    /// Seconds since unix epoch. A value of 0 represents no expiry.
    expiry: u64,
    /// Pointer to callable main function. Set to nullptr if account has no code.
    main: extern "C" fn(),
    storage_items: *const CStorageItem<'a>,
    num_storage_items: u64,
}

#[repr(C)]
pub struct CStorageItem<'a> {
    key: &'a CStr,
    value: &'a CStr,
}

type Memchain = RefCell<Blockchain<'static>>;

impl<'a> From<CAccount<'a>> for Account {
    fn from(ca: CAccount<'a>) -> Self {
        Self {
            balance: ca.balance,
            code: unsafe { slice::from_raw_parts(ca.code, ca.code_len as usize) }.to_vec(),
            storage: unsafe {
                slice::from_raw_parts(ca.storage_items, ca.num_storage_items as usize)
            }
            .iter()
            .map(|itm| (itm.key.to_bytes().to_vec(), itm.value.to_bytes().to_vec()))
            .collect(),
            expiry: if ca.expiry == 0 {
                None
            } else {
                Some(std::time::Duration::from_secs(ca.expiry))
            },
            main: if (ca.main as *const std::ffi::c_void).is_null() {
                None
            } else {
                Some(ca.main)
            },
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn create_memchain(
    genesis_accounts: *const CAccount,
    num_genesis_accounts: u32,
) -> *const Memchain {
    let genesis_state = slice::from_raw_parts(genesis_accounts, num_genesis_accounts as usize)
        .iter()
        .map(|ca| (ca.address, Cow::Owned(Account::from(*ca))))
        .collect();
    Rc::into_raw(Blockchain::new(genesis_state))
}

#[no_mangle]
pub unsafe extern "C" fn destroy_memchain(memchain: *mut Memchain) {
    std::mem::drop(Rc::from_raw(memchain))
}

/// Adds a new account to the blockchain at the current block.
/// Requires that a transaction is currently in progress.
/// Returns nonzero on error. An error will occur if the account already exists.
#[no_mangle]
pub unsafe extern "C" fn create_account(memchain: *const Memchain, new_account: CAccount) -> u8 {
    let rc_bc = Rc::from_raw(memchain);
    let mut bc = rc_bc.borrow_mut();
    let current_state = bc.current_state_mut();
    match current_state.entry(new_account.address) {
        Entry::Occupied(_) => 1,
        Entry::Vacant(v) => {
            v.insert(Cow::Owned(Account::from(new_account)));
            0
        }
    }
}
