use std::{
    borrow::Cow,
    cell::RefCell,
    ffi::CStr,
    io::{IoSlice, IoSliceMut},
    path::Path,
    rc::Rc,
    slice,
};

use memchain::{Account, Blockchain};
use oasis_types::{Address, U256};
use wasi_types::{ErrNo, Fd, FdFlags, FdStat, FileDelta, FileSize, FileStat, OpenFlags, Whence};

use crate::{BlockchainIntrinsics, KVStore, BCFS};

#[repr(C)]
pub struct GenesisAccount {
    address: Address,
    balance: U256,
}

#[no_mangle]
extern "C" fn create_memchain(
    genesis_accounts: *const GenesisAccount,
    num_genesis_accounts: u32,
) -> *mut Rc<RefCell<Blockchain<'static>>> {
    let genesis_state =
        unsafe { slice::from_raw_parts(genesis_accounts, num_genesis_accounts as usize) }
            .iter()
            .map(|GenesisAccount { address, balance }| {
                (*address, Cow::Owned(Account::new(*balance)))
            })
            .collect();
    &mut Rc::new(RefCell::new(Blockchain::new(genesis_state))) as *mut _
}

#[no_mangle]
extern "C" fn create_memory_bcfs(
    blockchain: *mut Rc<RefCell<Blockchain<'static>>>,
    owner_addr: Address,
) -> *mut BCFS {
    let bc = unsafe { &*blockchain };
    Box::into_raw(Box::new(BCFS::new(
        Rc::clone(bc) as Rc<RefCell<dyn KVStore>>,
        Rc::clone(bc) as Rc<RefCell<dyn BlockchainIntrinsics>>,
        owner_addr,
    )))
}

#[no_mangle]
extern "C" fn open(
    bcfs: *mut BCFS,
    path: &CStr,
    open_flags: OpenFlags,
    fd_flags: FdFlags,
    p_fd: *mut u32,
) -> ErrNo {
    let bcfs = Box::leak(unsafe { Box::from_raw(bcfs) });
    let path = Path::new(match path.to_str() {
        Ok(path) => path,
        Err(_) => return ErrNo::Inval,
    });
    match bcfs.open(None /* curdir */, path, open_flags, fd_flags) {
        Ok(fd) => {
            unsafe { *p_fd = fd.into() };
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
extern "C" fn close(bcfs: *mut BCFS, fd: Fd) -> ErrNo {
    let bcfs = Box::leak(unsafe { Box::from_raw(bcfs) });
    match bcfs.close(fd.into()) {
        Ok(_) => ErrNo::Success,
        Err(err) => err,
    }
}

#[no_mangle]
extern "C" fn seek(
    bcfs: *mut BCFS,
    fd: Fd,
    offset: FileDelta,
    whence: Whence,
    p_offset: *mut FileSize,
) -> ErrNo {
    let bcfs = Box::leak(unsafe { Box::from_raw(bcfs) });
    match bcfs.seek(fd, offset, whence) {
        Ok(offset) => {
            unsafe { *p_offset = offset }
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
extern "C" fn fdstat(bcfs: *mut BCFS, fd: Fd, p_fdstat: *mut FdStat) -> ErrNo {
    let bcfs = Box::leak(unsafe { Box::from_raw(bcfs) });
    match bcfs.fdstat(fd) {
        Ok(fdstat) => {
            unsafe { *p_fdstat = fdstat }
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
extern "C" fn filestat(bcfs: *mut BCFS, fd: Fd, p_filestat: *mut FileStat) -> ErrNo {
    let bcfs = Box::leak(unsafe { Box::from_raw(bcfs) });
    match bcfs.filestat(fd) {
        Ok(filestat) => {
            unsafe { *p_filestat = filestat }
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[repr(C)]
pub struct IoVec {
    pub buf: *const u8,
    pub len: usize,
}

#[repr(C)]
pub struct IoVecMut {
    pub buf: *mut u8,
    pub len: usize,
}

fn do_read<F: FnOnce(&mut BCFS, &mut [IoSliceMut]) -> crate::Result<usize>>(
    bcfs: *mut BCFS,
    fd: Fd,
    iovs: *mut IoVecMut,
    num_iovs: usize,
    p_nbytes: *mut FileSize,
    read_fn: F,
) -> ErrNo {
    let bcfs = Box::leak(unsafe { Box::from_raw(bcfs) });
    let mut bufs = unsafe { slice::from_raw_parts_mut(iovs, num_iovs as usize) }
        .iter()
        .map(|iov| IoSliceMut::new(unsafe { slice::from_raw_parts_mut(iov.buf, iov.len) }))
        .collect::<Vec<_>>();
    match read_fn(bcfs, &mut bufs) {
        Ok(nbytes) => {
            unsafe { *p_nbytes = nbytes as FileSize }
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
extern "C" fn read_vectored(
    bcfs: *mut BCFS,
    fd: Fd,
    iovs: *mut IoVecMut,
    num_iovs: usize,
    p_nbytes: *mut FileSize,
) -> ErrNo {
    do_read(bcfs, fd, iovs, num_iovs, p_nbytes, |bcfs, bufs| {
        bcfs.read_vectored(fd, bufs)
    })
}

#[no_mangle]
extern "C" fn pread_vectored(
    bcfs: *mut BCFS,
    fd: Fd,
    iovs: *mut IoVecMut,
    num_iovs: usize,
    offset: FileSize,
    p_nbytes: *mut FileSize,
) -> ErrNo {
    do_read(bcfs, fd, iovs, num_iovs, p_nbytes, |bcfs, bufs| {
        bcfs.pread_vectored(fd, bufs, offset)
    })
}

fn do_write<F: FnOnce(&mut BCFS, &[IoSlice]) -> crate::Result<usize>>(
    bcfs: *mut BCFS,
    fd: Fd,
    iovs: *const IoVec,
    num_iovs: usize,
    p_nbytes: *mut FileSize,
    write_fn: F,
) -> ErrNo {
    let bcfs = Box::leak(unsafe { Box::from_raw(bcfs) });
    let mut bufs = unsafe { slice::from_raw_parts(iovs, num_iovs as usize) }
        .iter()
        .map(|iov| IoSlice::new(unsafe { slice::from_raw_parts(iov.buf, iov.len) }))
        .collect::<Vec<_>>();
    match write_fn(bcfs, &bufs) {
        Ok(nbytes) => {
            unsafe { *p_nbytes = nbytes as FileSize }
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
extern "C" fn write_vectored(
    bcfs: *mut BCFS,
    fd: Fd,
    iovs: *const IoVec,
    num_iovs: usize,
    p_nbytes: *mut FileSize,
) -> ErrNo {
    do_write(bcfs, fd, iovs, num_iovs, p_nbytes, |bcfs, bufs| {
        bcfs.write_vectored(fd, bufs)
    })
}

#[no_mangle]
extern "C" fn pwrite_vectored(
    bcfs: *mut BCFS,
    fd: Fd,
    iovs: *const IoVec,
    num_iovs: usize,
    offset: FileSize,
    p_nbytes: *mut FileSize,
) -> ErrNo {
    do_write(bcfs, fd, iovs, num_iovs, p_nbytes, |bcfs, bufs| {
        bcfs.pwrite_vectored(fd, bufs, offset)
    })
}
