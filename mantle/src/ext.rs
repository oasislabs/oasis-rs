use mantle_types::Address;

use crate::error::Error;

#[repr(C)]
#[derive(PartialEq, Eq)]
pub struct StatusCode(pub u32);

#[allow(non_upper_case_globals)] // it's supposed to be a non-exhaustive enum
impl StatusCode {
    pub const Success: StatusCode = StatusCode(0);
    pub const InsufficientFunds: StatusCode = StatusCode(1);
    pub const OutOfGas: StatusCode = StatusCode(2);
    pub const NoAccount: StatusCode = StatusCode(3);
}

mod ext {
    use super::*;

    /// @see the `blockchain-traits` crate for descriptions of these methods.
    extern "C" {
        pub fn balance(addr: *const Address, balance: *mut u64) -> StatusCode;

        pub fn code(addr: *const Address, buf: *mut u8) -> StatusCode;
        pub fn code_len(at: *const Address, len: *mut u32) -> StatusCode;

        pub fn fetch_input(buf: *mut u8) -> StatusCode;
        pub fn input_len(len: *mut u32) -> StatusCode;

        pub fn ret(buf: *const u8, len: u32) -> StatusCode;
        pub fn err(buf: *const u8, len: u32) -> StatusCode;

        pub fn fetch_ret(buf: *mut u8) -> StatusCode;
        pub fn ret_len(len: *mut u32) -> StatusCode;

        pub fn fetch_err(buf: *mut u8) -> StatusCode;
        pub fn err_len(len: *mut u32) -> StatusCode;

        pub fn transact(
            callee: *const Address,
            value: u64,
            input: *const u8,
            input_len: u32,
        ) -> StatusCode;

        pub fn address(addr: *mut Address) -> StatusCode;
        pub fn sender(addr: *mut Address) -> StatusCode;
        pub fn value(value: *mut u64) -> StatusCode;
    }
}

macro_rules! ext {
    ($fn:ident $args:tt ) => {{
        let outcome = unsafe { ext::$fn$args };
        if outcome != StatusCode::Success {
            Err(Error::from(outcome))
        } else {
            Ok(())
        }
    }}
}

pub fn code(at: &Address) -> Option<Vec<u8>> {
    let mut code_len = 0u32;
    let mut code = Vec::with_capacity(
        match ext!(code_len(at as *const Address, &mut code_len as *mut _)) {
            Ok(_) => code_len as usize,
            Err(_) => return None,
        },
    );
    ext!(code(at as *const Address, code.as_mut_ptr()))
        .ok()
        .map(|_| code)
}

pub fn address() -> Address {
    let mut addr = Address::default();
    ext!(address(&mut addr as *mut _)).unwrap();
    addr
}

pub fn balance(addr: &Address) -> Option<u64> {
    let mut balance = 0;
    ext!(balance(addr as *const _, &mut balance as *mut _))
        .ok()
        .map(|_| balance)
}

pub fn sender() -> Address {
    let mut addr = Address::default();
    ext!(sender(&mut addr as *mut _)).unwrap();
    addr
}

pub fn value() -> u64 {
    let mut value = 0;
    ext!(value(&mut value as *mut _)).unwrap();
    value
}

pub fn transact(callee: &Address, value: u64, input: Vec<u8>) -> Result<Vec<u8>, Error> {
    ext!(transact(
        callee as *const _,
        value,
        input.as_ptr(),
        if input.len() > u32::max_value() as usize {
            return Err(Error::InvalidInput);
        } else {
            input.len() as u32
        },
    ))?;

    let mut ret_len = 0u32;
    unsafe { ext::ret_len(&mut ret_len as *mut _) };

    let mut ret = Vec::with_capacity(ret_len as usize);
    unsafe { ret.set_len(ret_len as usize) };

    ext!(fetch_ret(ret.as_mut_ptr())).map(|_| ret)
}

pub fn transfer(to: &Address, value: u64) -> Result<(), Error> {
    ext!(transact(to as *const _, value, std::ptr::null(), 0))
}

pub fn fetch_input() -> Vec<u8> {
    let mut input_len = 0u32;
    unsafe { ext::input_len(&mut input_len as *mut _) };

    let mut input = Vec::with_capacity(input_len as usize);
    unsafe { input.set_len(input_len as usize) };

    ext!(fetch_input(input.as_mut_ptr())).unwrap();
    input
}

pub fn fetch_ret() -> Vec<u8> {
    let mut ret_len = 0u32;
    unsafe { ext::ret_len(&mut ret_len as *mut _) };

    let mut ret = Vec::with_capacity(ret_len as usize);
    unsafe { ret.set_len(ret_len as usize) };

    ext!(fetch_ret(ret.as_mut_ptr())).unwrap();
    ret
}

pub fn fetch_err() -> Vec<u8> {
    let mut err_len = 0u32;
    unsafe { ext::err_len(&mut err_len as *mut _) };

    let mut err = Vec::with_capacity(err_len as usize);
    unsafe { err.set_len(err_len as usize) };

    ext!(fetch_err(err.as_mut_ptr())).unwrap();
    err
}

pub fn ret(ret: Vec<u8>) {
    ext!(ret(ret.as_ptr(), ret.len() as u32)).unwrap();
}

pub fn err(err: Vec<u8>) {
    ext!(err(err.as_ptr(), err.len() as u32)).unwrap();
}
