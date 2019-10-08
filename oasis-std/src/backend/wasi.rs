use std::{
    fs,
    io::{self, Read as _, Write as _},
    os::wasi::{ffi::OsStringExt, io::FromRawFd},
    path::{Path, PathBuf},
    str::FromStr,
};

use libc::{__wasi_errno_t, __wasi_fd_t};
use oasis_types::{Address, Balance};

use super::Error;

macro_rules! chain_dir {
    ($($ext:literal),*) => {
        concat!("/opt/oasis/", $($ext),*)
    }
}
fn home<P: AsRef<Path>>(addr: &Address, file: P) -> PathBuf {
    let mut home = PathBuf::from(chain_dir!());
    home.push(addr.path_repr());
    home.push(file.as_ref());
    home
}

fn env_addr(key: &str) -> Address {
    let mut addr = Address::default();
    addr.0
        .copy_from_slice(&hex::decode(&std::env::var_os(key).unwrap().into_vec()).unwrap());
    addr
}

pub fn address() -> Address {
    env_addr("ADDRESS")
}

pub fn sender() -> Address {
    env_addr("SENDER")
}

pub fn payer() -> Address {
    env_addr("PAYER")
}

pub fn aad() -> Vec<u8> {
    base64::decode(&std::env::var_os("AAD").unwrap().into_vec()).unwrap()
}

pub fn value() -> Balance {
    Balance(u128::from_str(&std::env::var("VALUE").unwrap()).unwrap())
}

pub fn balance(addr: &Address) -> Option<Balance> {
    Some(match fs::read(home(&*addr, "balance")) {
        Ok(balance) => {
            let mut buf = [0u8; 16];
            buf.copy_from_slice(&balance);
            Balance(u128::from_le_bytes(buf))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(err) => panic!(err),
    })
}

pub fn code(addr: &Address) -> Option<Vec<u8>> {
    Some(match fs::read(home(&*addr, "code")) {
        Ok(code) => code,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(err) => panic!(err),
    })
}

#[link(wasm_import_module = "wasi_unstable")]
extern "C" {
    #[link_name = "blockchain_transact"]
    #[allow(improper_ctypes)] // u128 is just 2 u64s
    fn __wasi_blockchain_transact(
        callee_addr: *const u8,
        value: *const u128,
        input: *const u8,
        input_len: u64,
        fd: *mut __wasi_fd_t,
    ) -> __wasi_errno_t;
}

pub fn transact(callee: &Address, value: Balance, input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut fd: __wasi_fd_t = 0;
    let errno = unsafe {
        __wasi_blockchain_transact(
            callee.0.as_ptr(),
            &value.0 as *const u128,
            input.as_ptr(),
            input.len() as u64,
            &mut fd as *mut _,
        )
    };
    let mut f_out = unsafe { fs::File::from_raw_fd(fd) };
    let mut out = Vec::new();
    f_out
        .read_to_end(&mut out)
        .unwrap_or_else(|err| panic!(err));
    match errno {
        libc::__WASI_ESUCCESS => Ok(out),
        libc::__WASI_EFAULT | libc::__WASI_EINVAL => Err(Error::InvalidInput),
        libc::__WASI_ENOENT => Err(Error::InvalidCallee),
        libc::__WASI_EDQUOT => Err(Error::InsufficientFunds),
        libc::__WASI_ECONNABORTED => Err(Error::Execution { payload: out }),
        _ => Err(Error::Unknown),
    }
}

pub fn input() -> Vec<u8> {
    let mut inp = Vec::new();
    io::stdin().read_to_end(&mut inp).unwrap();
    inp
}

pub fn ret(ret: &[u8]) -> ! {
    io::stdout().write_all(&ret).unwrap();
    std::process::exit(0);
}

pub fn err(err: &[u8]) -> ! {
    io::stdout().write_all(&err).unwrap();
    std::process::exit(1);
}

pub fn read(key: &[u8]) -> Vec<u8> {
    fs::read(std::str::from_utf8(key).unwrap()).unwrap_or_default()
}

pub fn write(key: &[u8], value: &[u8]) {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(std::str::from_utf8(key).unwrap())
        .unwrap();
    f.write_all(value).unwrap();
}

pub fn emit(topics: &[&[u8]], data: &[u8]) {
    let mut f_log = fs::OpenOptions::new()
        .append(true)
        .open(chain_dir!("log"))
        .unwrap();
    f_log
        .write_all(&(topics.len() as u32).to_le_bytes())
        .unwrap();
    for topic in topics {
        f_log
            .write_all(&(topic.len() as u32).to_le_bytes())
            .unwrap();
        f_log.write_all(topic).unwrap();
    }
    f_log.write_all(&(data.len() as u32).to_le_bytes());
    f_log.write_all(data).unwrap();
}
