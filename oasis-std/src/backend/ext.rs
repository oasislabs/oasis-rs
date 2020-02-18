use oasis_types::{Address, Balance, ExtStatusCode, RpcError};

/// @see the `blockchain-traits` crate for descriptions of these methods.
extern "C" {
    #[allow(improper_ctypes)] // u128 is just 2 u64s
    pub fn oasis_balance(addr: *const Address, balance: *mut u128) -> u32;

    pub fn oasis_code(addr: *const Address, buf: *mut u8) -> u32;
    pub fn oasis_code_len(addr: *const Address, len: *mut u32) -> u32;

    pub fn oasis_fetch_input(buf: *mut u8) -> u32;
    pub fn oasis_input_len(len: *mut u32) -> u32;

    pub fn oasis_ret(buf: *const u8, len: u32) -> u32;
    pub fn oasis_err(buf: *const u8, len: u32) -> u32;

    pub fn oasis_fetch_ret(buf: *mut u8) -> u32;
    pub fn oasis_ret_len(len: *mut u32) -> u32;

    pub fn oasis_fetch_err(buf: *mut u8) -> u32;
    pub fn oasis_err_len(len: *mut u32) -> u32;

    pub fn oasis_fetch_aad(buf: *mut u8) -> u32;
    pub fn oasis_aad_len(len: *mut u32) -> u32;

    #[allow(improper_ctypes)] // u128 is just 2 u64s
    pub fn oasis_transact(
        callee: *const Address,
        value: *const u128,
        input: *const u8,
        input_len: u32,
    ) -> u32;

    pub fn oasis_address(addr: *mut Address) -> u32;
    pub fn oasis_sender(addr: *mut Address) -> u32;
    pub fn oasis_payer(addr: *mut Address) -> u32;
    #[allow(improper_ctypes)] // u128 is just 2 u64s
    pub fn oasis_value(value: *mut u128) -> u32;

    pub fn oasis_read(key: *const u8, key_len: u32, value: *mut u8) -> u32;
    pub fn oasis_read_len(key: *const u8, key_len: u32, value_len: *mut u32) -> u32;
    pub fn oasis_write(key: *const u8, key_len: u32, value: *const u8, value_len: u32) -> u32;

    pub fn oasis_emit(
        topics: *const *const u8,
        topic_lens: *const u32,
        num_topics: u32,
        data: *const u8,
        data_len: u32,
    ) -> u32;
}

fn unpack_rpc_error(status: u32) -> RpcError {
    match ExtStatusCode::from_u32(status) {
        Some(ExtStatusCode::Success) => unreachable!(),
        Some(ExtStatusCode::InsufficientFunds) => RpcError::InsufficientFunds,
        Some(ExtStatusCode::InvalidInput) => RpcError::InvalidInput,
        Some(ExtStatusCode::NoAccount) => RpcError::InvalidCallee,
        Some(_) | None => RpcError::Execution(fetch_err()),
    }
}

macro_rules! ext {
    ($fn:ident $args:tt ) => {{
        let code = unsafe { $fn$args };
        if code != ExtStatusCode::Success as u32 {
            Err(code)
        } else {
            Ok(())
        }
    }};
}

pub fn address() -> Address {
    let mut addr = Address::default();
    ext!(oasis_address(&mut addr as *mut _)).unwrap();
    addr
}

pub fn sender() -> Address {
    let mut addr = Address::default();
    ext!(oasis_sender(&mut addr as *mut _)).unwrap();
    addr
}

pub fn payer() -> Address {
    let mut addr = Address::default();
    ext!(oasis_payer(&mut addr as *mut _)).unwrap();
    addr
}

pub fn aad() -> Vec<u8> {
    let mut aad_len = 0u32;
    ext!(oasis_aad_len(&mut aad_len as *mut _)).unwrap();

    let mut aad = Vec::with_capacity(aad_len as usize);
    unsafe { aad.set_len(aad_len as usize) };

    ext!(oasis_fetch_aad(aad.as_mut_ptr())).unwrap();
    aad
}

pub fn value() -> Balance {
    let mut value = 0;
    ext!(oasis_value(&mut value as *mut _)).unwrap();
    Balance(value)
}

pub fn balance(addr: &Address) -> Option<Balance> {
    let mut balance = 0;
    ext!(oasis_balance(addr as *const _, &mut balance as *mut _))
        .ok()
        .map(|_| Balance(balance))
}

pub fn code(addr: &Address) -> Option<Vec<u8>> {
    let mut code_len = 0u32;
    let mut code = Vec::with_capacity(
        match ext!(oasis_code_len(
            addr as *const Address,
            &mut code_len as *mut _
        )) {
            Ok(_) => code_len as usize,
            Err(_) => return None,
        },
    );
    ext!(oasis_code(addr as *const Address, code.as_mut_ptr()))
        .ok()
        .map(|_| code)
}

pub fn transact(callee: &Address, value: Balance, input: &[u8]) -> Result<Vec<u8>, RpcError> {
    ext!(oasis_transact(
        callee as *const _,
        &value.0 as *const u128,
        input.as_ptr(),
        if input.len() > u32::max_value() as usize {
            return Err(RpcError::InvalidInput);
        } else {
            input.len() as u32
        },
    ))
    .map_err(unpack_rpc_error)?;

    let mut ret_len = 0u32;
    ext!(oasis_ret_len(&mut ret_len as *mut _)).map_err(unpack_rpc_error)?;

    let mut ret = Vec::with_capacity(ret_len as usize);
    unsafe { ret.set_len(ret_len as usize) };

    ext!(oasis_fetch_ret(ret.as_mut_ptr()))
        .map(|_| ret)
        .map_err(unpack_rpc_error)
}

pub fn input() -> Vec<u8> {
    let mut input_len = 0u32;
    ext!(oasis_input_len(&mut input_len as *mut _)).unwrap();

    let mut input = Vec::with_capacity(input_len as usize);
    unsafe { input.set_len(input_len as usize) };

    ext!(oasis_fetch_input(input.as_mut_ptr())).unwrap();
    input
}

pub fn ret(ret: &[u8]) -> ! {
    ext!(oasis_ret(ret.as_ptr(), ret.len() as u32)).unwrap();
    std::process::abort();
}

pub fn err(err: &[u8]) -> ! {
    ext!(oasis_err(err.as_ptr(), err.len() as u32)).unwrap();
    std::process::abort();
}

pub fn fetch_err() -> Vec<u8> {
    let mut err_len = 0u32;
    ext!(oasis_err_len(&mut err_len as *mut _)).unwrap();

    let mut err = Vec::with_capacity(err_len as usize);
    unsafe { err.set_len(err_len as usize) };

    ext!(oasis_fetch_err(err.as_mut_ptr())).unwrap();
    err
}

pub fn read(key: &[u8]) -> Vec<u8> {
    let mut val_len = 0u32;
    ext!(oasis_read_len(
        key.as_ptr(),
        key.len() as u32,
        &mut val_len as *mut _
    ))
    .unwrap();

    let mut val = Vec::with_capacity(val_len as usize);
    unsafe { val.set_len(val_len as usize) };

    ext!(oasis_read(key.as_ptr(), key.len() as u32, val.as_mut_ptr())).unwrap();
    val
}

pub fn write(key: &[u8], value: &[u8]) {
    ext!(oasis_write(
        key.as_ptr(),
        key.len() as u32,
        value.as_ptr(),
        value.len() as u32
    ))
    .unwrap();
}

pub fn emit(topics: &[&[u8]], data: &[u8]) {
    let topic_ptrs: Vec<*const u8> = topics.iter().map(|t| t.as_ptr()).collect();
    let topic_lens: Vec<u32> = topics.iter().map(|t| t.len() as u32).collect();
    ext!(oasis_emit(
        topic_ptrs.as_ptr(),
        topic_lens.as_ptr(),
        topics.len() as u32,
        data.as_ptr(),
        data.len() as u32
    ))
    .unwrap();
}
