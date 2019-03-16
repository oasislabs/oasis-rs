//! Safe wrappers around interpreter intrinsics.

use crate::{errors::ExtCallError, types::*};

mod eth {
    extern "C" {
        /// Direct/classic call. Corresponds to "CALL" opcode in EVM
        pub fn ccall(
            gas: u64,
            address: *const u8,
            val_ptr: *const u8,
            input_ptr: *const u8,
            input_len: u32,
        ) -> i32;

        /// Delegate call. Corresponds to "CALLCODE" opcode in EVM
        pub fn dcall(gas: u64, address: *const u8, input_ptr: *const u8, input_len: u32) -> i32;

        /// Static call. Corresponds to "STACICCALL" opcode in EVM
        pub fn scall(gas: u64, address: *const u8, input_ptr: *const u8, input_len: u32) -> i32;

        // blockchain functions
        pub fn address(dest: *mut u8);
        pub fn balance(address: *const u8, dest: *mut u8);
        pub fn blockhash(number: i64, dest: *mut u8);
        pub fn blocknumber() -> i64;
        pub fn coinbase(dest: *mut u8);
        pub fn create(
            endowment: *const u8,
            code_ptr: *const u8,
            code_len: u32,
            result_ptr: *mut u8,
        ) -> i32;
        #[cfg(feature = "create2")]
        pub fn create2(
            endowment: *const u8,
            salt: *const u8,
            code_ptr: *const u8,
            code_len: u32,
            result_ptr: *mut u8,
        ) -> i32;
        pub fn difficulty(dest: *mut u8);
        pub fn elog(topic_ptr: *const u8, topic_count: u32, data_ptr: *const u8, data_len: u32);
        pub fn input_length() -> u32;
        pub fn fetch_input(dest: *mut u8);
        pub fn gasleft() -> i64;
        pub fn gaslimit(dest: *mut u8);
        pub fn origin(dest: *mut u8);
        pub fn ret(ptr: *const u8, len: u32); // -> !
        pub fn sender(dest: *mut u8);
        pub fn suicide(refund: *const u8); //  -> !
        pub fn timestamp() -> i64;
        pub fn value(dest: *mut u8);
        pub fn return_length() -> u32;
        pub fn fetch_return(dest: *mut u8);
    }
}

mod oasis {
    extern "C" {
        // oasis platform functions
        pub fn storage_read(key: *const u8, dest: *mut u8);
        pub fn storage_write(key: *const u8, src: *const u8);

        // Key must be 32 bytes.
        pub fn get_bytes(key: *const u8, result: *mut u8);
        // Key must be 32 bytes.
        pub fn get_bytes_len(key: *const u8) -> u64;
        // Key must be 32 bytes.
        pub fn set_bytes(key: *const u8, bytes: *const u8, bytes_len: u64);
    }
}

/// Halt execution and register account for deletion.
///
/// Value of the current account will be tranfered to `refund` address.
/// Runtime SHOULD trap the execution.
pub fn suicide(refund: &Address) {
    unsafe {
        eth::suicide(refund.as_ptr());
    }
}

/// Get balance of the given account.
///
/// If an account is not registered in the chain yet,
/// it is considered as an account with `balance = 0`.
pub fn balance(address: &Address) -> U256 {
    unsafe { fetch_u256(|x| eth::balance(address.as_ptr(), x)) }
}

/// Create a new account with the given code
/// Returns an error if the contract constructor failed.
pub fn create(endowment: U256, code: &[u8]) -> Result<Address, ExtCallError> {
    let mut endowment_arr = [0u8; 32];
    endowment.to_big_endian(&mut endowment_arr);
    let mut result = Address::zero();
    unsafe {
        if eth::create(
            endowment_arr.as_ptr(),
            code.as_ptr(),
            code.len() as u32,
            (&mut result).as_mut_ptr(),
        ) == 0
        {
            Ok(result)
        } else {
            Err(ExtCallError)
        }
    }
}

/// Create a new account with the given code and salt.
/// Returns an error if the contract constructor failed.
#[cfg(feature = "create2")]
pub fn create2(endowment: U256, salt: H256, code: &[u8]) -> Result<Address, ExtCallError> {
    let mut endowment_arr = [0u8; 32];
    endowment.to_big_endian(&mut endowment_arr);
    let mut result = Address::zero();
    unsafe {
        if eth::create2(
            endowment_arr.as_ptr(),
            salt.as_ptr(),
            code.as_ptr(),
            code.len() as u32,
            (&mut result).as_mut_ptr(),
        ) == 0
        {
            Ok(result)
        } else {
            Err(ExtCallError)
        }
    }
}

/// Message-call into an account
///
///  # Arguments:
/// * `gas`- a gas limit for a call. A call execution will halt if call exceed this amount
/// * `address` - an address of contract to send a call
/// * `value` - a value in Wei to send with a call
/// * `input` - a data to send with a call
/// * `result` - a mutable reference to be filled with a result data
pub fn call(
    gas: u64,
    address: &Address,
    value: U256,
    input: &[u8],
) -> Result<Vec<u8>, ExtCallError> {
    let mut value_arr = [0u8; 32];
    value.to_big_endian(&mut value_arr);
    unsafe {
        if eth::ccall(
            gas,
            address.as_ptr(),
            value_arr.as_ptr(),
            input.as_ptr(),
            input.len() as u32,
        ) == 0
        {
            let mut result = vec![0u8; eth::return_length() as usize];
            eth::fetch_return(result.as_mut_ptr());
            Ok(result)
        } else {
            Err(ExtCallError)
        }
    }
}

/// Like `call`, but with code at the given `address`
///
/// Effectively this function is like calling current account but with
/// different code (i.e. like `DELEGATECALL` EVM instruction).
pub fn call_code(gas: u64, address: &Address, input: &[u8]) -> Result<Vec<u8>, ExtCallError> {
    unsafe {
        if eth::dcall(gas, address.as_ptr(), input.as_ptr(), input.len() as u32) == 0 {
            let mut result = vec![0u8; eth::return_length() as usize];
            eth::fetch_return(result.as_mut_ptr());
            Ok(result)
        } else {
            Err(ExtCallError)
        }
    }
}

/// Like `call`, but this call and any of it's subcalls are disallowed to modify any storage.
/// It will return an error in this case.
pub fn static_call(gas: u64, address: &Address, input: &[u8]) -> Result<Vec<u8>, ExtCallError> {
    unsafe {
        if eth::scall(gas, address.as_ptr(), input.as_ptr(), input.len() as u32) == 0 {
            let mut result = vec![0u8; eth::return_length() as usize];
            eth::fetch_return(result.as_mut_ptr());
            Ok(result)
        } else {
            Err(ExtCallError)
        }
    }
}

/// Returns hash of the given block or H256::zero()
///
/// Only works for 256 most recent blocks excluding current
/// Returns H256::zero() in case of failure
pub fn block_hash(block_number: u64) -> H256 {
    let mut res = H256::zero();
    unsafe { eth::blockhash(block_number as i64, res.as_mut_ptr()) }
    res
}

/// Get the current block’s beneficiary address (the current miner account address)
pub fn coinbase() -> Address {
    unsafe { fetch_address(|x| eth::coinbase(x)) }
}

/// Get the block's timestamp
///
/// It can be viewed as an output of Unix's `time()` function at
/// current block's inception.
pub fn timestamp() -> u64 {
    unsafe { eth::timestamp() as u64 }
}

/// Get the block's number
///
/// This value represents number of ancestor blocks.
/// The genesis block has a number of zero.
pub fn block_number() -> u64 {
    unsafe { eth::blocknumber() as u64 }
}

/// Get the block's difficulty.
pub fn difficulty() -> U256 {
    unsafe { fetch_u256(|x| eth::difficulty(x)) }
}

/// Get the block's gas limit.
pub fn gas_limit() -> U256 {
    unsafe { fetch_u256(|x| eth::gaslimit(x)) }
}

/// Get amount of gas left.
pub fn gas_left() -> u64 {
    unsafe { eth::gasleft() as u64 }
}

/// Get caller address
///
/// This is the address of the account that is directly responsible for this execution.
/// Use `origin` to get an address of external account - an original initiator of a transaction
pub fn sender() -> Address {
    unsafe { fetch_address(|x| eth::sender(x)) }
}

/// Get execution origination address
///
/// This is the sender of original transaction.
/// It could be only external account, not a contract
pub fn origin() -> Address {
    unsafe { fetch_address(|x| eth::origin(x)) }
}

/// Get deposited value by the instruction/transaction responsible for this execution.
pub fn value() -> U256 {
    unsafe { fetch_u256(|x| eth::value(x)) }
}

/// Get address of currently executing account
pub fn address() -> Address {
    unsafe { fetch_address(|x| eth::address(x)) }
}

/// Creates log entry with up to four topics and data.
///
/// # Panics
/// If `topics` contains more than 4 elements then this function will trap.
pub fn log(topics: &[H256], data: &[u8]) {
    unsafe {
        eth::elog(
            topics.as_ptr() as *const u8,
            topics.len() as u32,
            data.as_ptr(),
            data.len() as u32,
        );
    }
}

/// Allocates and requests `call` arguments (input)
/// Input data comes either with external transaction or from `call` input value.
pub fn input() -> Vec<u8> {
    match unsafe { eth::input_length() } {
        0 => Vec::new(),
        len => {
            let mut data = vec![0; len as usize];
            unsafe {
                eth::fetch_input(data.as_mut_ptr());
            }
            data
        }
    }
}

/// Sets a `call` return value
/// Pass return data to the runtime. Runtime SHOULD trap the execution.
pub fn ret(data: &[u8]) {
    unsafe {
        eth::ret(data.as_ptr(), data.len() as u32);
    }
}

/// Performs read from the storage.
pub fn read(key: &H256) -> [u8; 32] {
    let mut dest = [0u8; 32];
    unsafe {
        oasis::storage_read(key.as_ptr(), dest.as_mut_ptr());
    }
    dest
}

/// Performs write to the storage
pub fn write(key: &H256, val: &[u8; 32]) {
    unsafe {
        oasis::storage_write(key.as_ptr(), val.as_ptr());
    }
}

/// Retrieve data directly from the contract storage trie.
pub fn get_bytes(key: &H256) -> Result<Vec<u8>, ExtCallError> {
    let result_len = get_bytes_len(key)?;
    let mut result = vec![0; result_len as usize];
    unsafe {
        oasis::get_bytes(key.as_ptr(), result.as_mut_ptr());
    }
    Ok(result)
}

fn get_bytes_len(key: &H256) -> Result<u32, ExtCallError> {
    unsafe { Ok(oasis::get_bytes_len(key.as_ptr()) as u32) }
}

/// Store data directly into the contract storage trie.
pub fn set_bytes<T: AsRef<[u8]>>(key: &H256, bytes: T) -> Result<(), ExtCallError> {
    let bytes = bytes.as_ref();
    let len = bytes.len() as u64;
    unsafe {
        oasis::set_bytes(key.as_ptr(), bytes.as_ptr(), len);
    }
    Ok(())
}

unsafe fn fetch_address<F: Fn(*mut u8)>(f: F) -> Address {
    let mut res = Address::zero();
    f(res.as_mut_ptr());
    res
}

unsafe fn fetch_u256<F: Fn(*mut u8)>(f: F) -> U256 {
    let mut res = [0u8; 32];
    f(res.as_mut_ptr());
    U256::from_big_endian(&res)
}
