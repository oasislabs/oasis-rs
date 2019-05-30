use std::{
    ffi::CStr,
    io::{IoSlice, IoSliceMut},
    path::Path,
    slice,
};

use blockchain_traits::Blockchain;
use mantle_types::Address;
use wasi_types::{ErrNo, Fd, FdFlags, FdStat, FileDelta, FileSize, FileStat, OpenFlags, Whence};

use crate::BCFS;

#[no_mangle]
pub unsafe extern "C" fn create_bcfs(
    bc: *const *mut dyn Blockchain<Address = Address>,
    owner_addr: Address,
) -> *mut BCFS<Address> {
    Box::into_raw(Box::new(BCFS::new(&mut *(*bc), owner_addr)))
}

#[no_mangle]
pub unsafe extern "C" fn destroy_bcfs(bcfs: *mut BCFS<Address>) {
    std::mem::drop(Box::from_raw(bcfs))
}

#[no_mangle]
pub unsafe extern "C" fn open(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    path: &CStr,
    open_flags: OpenFlags,
    fd_flags: FdFlags,
    p_fd: *mut u32,
) -> ErrNo {
    let bcfs = Box::leak(Box::from_raw(bcfs));
    let path = Path::new(match path.to_str() {
        Ok(path) => path,
        Err(_) => return ErrNo::Inval,
    });
    match bcfs.open(
        &mut *(*bc),
        None, /* curdir */
        path,
        open_flags,
        fd_flags,
    ) {
        Ok(fd) => {
            *p_fd = fd.into();
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
pub unsafe extern "C" fn close(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    fd: Fd,
) -> ErrNo {
    let bcfs = Box::leak(Box::from_raw(bcfs));
    match bcfs.close(&mut *(*bc), fd) {
        Ok(_) => ErrNo::Success,
        Err(err) => err,
    }
}

#[no_mangle]
pub unsafe extern "C" fn seek(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    fd: Fd,
    offset: FileDelta,
    whence: Whence,
    p_offset: *mut FileSize,
) -> ErrNo {
    let bcfs = Box::leak(Box::from_raw(bcfs));
    match bcfs.seek(&mut *(*bc), fd, offset, whence) {
        Ok(offset) => {
            *p_offset = offset;
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
pub unsafe extern "C" fn fdstat(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    fd: Fd,
    p_fdstat: *mut FdStat,
) -> ErrNo {
    let bcfs = Box::leak(Box::from_raw(bcfs));
    match bcfs.fdstat(&mut *(*bc), fd) {
        Ok(fdstat) => {
            *p_fdstat = fdstat;
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
pub unsafe extern "C" fn filestat(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    fd: Fd,
    p_filestat: *mut FileStat,
) -> ErrNo {
    let bcfs = Box::leak(Box::from_raw(bcfs));
    match bcfs.filestat(&mut *(*bc), fd) {
        Ok(filestat) => {
            *p_filestat = filestat;
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

unsafe fn do_read<
    F: FnOnce(
        &mut BCFS<Address>,
        &mut dyn Blockchain<Address = Address>,
        &mut [IoSliceMut],
    ) -> crate::Result<usize>,
>(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    iovs: *mut IoVecMut,
    num_iovs: usize,
    p_nbytes: *mut FileSize,
    read_fn: F,
) -> ErrNo {
    let bcfs = Box::leak(Box::from_raw(bcfs));
    let mut bufs = slice::from_raw_parts_mut(iovs, num_iovs as usize)
        .iter()
        .map(|iov| IoSliceMut::new(slice::from_raw_parts_mut(iov.buf, iov.len)))
        .collect::<Vec<_>>();
    match read_fn(bcfs, &mut *(*bc), &mut bufs) {
        Ok(nbytes) => {
            *p_nbytes = nbytes as FileSize;
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
pub unsafe extern "C" fn read_vectored(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    fd: Fd,
    iovs: *mut IoVecMut,
    num_iovs: usize,
    p_nbytes: *mut FileSize,
) -> ErrNo {
    do_read(bcfs, bc, iovs, num_iovs, p_nbytes, |bcfs, bc, bufs| {
        bcfs.read_vectored(bc, fd, bufs)
    })
}

#[no_mangle]
pub unsafe extern "C" fn pread_vectored(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    fd: Fd,
    iovs: *mut IoVecMut,
    num_iovs: usize,
    offset: FileSize,
    p_nbytes: *mut FileSize,
) -> ErrNo {
    do_read(bcfs, bc, iovs, num_iovs, p_nbytes, |bcfs, bc, bufs| {
        bcfs.pread_vectored(bc, fd, bufs, offset)
    })
}

unsafe fn do_write<
    F: FnOnce(
        &mut BCFS<Address>,
        &mut Blockchain<Address = Address>,
        &[IoSlice],
    ) -> crate::Result<usize>,
>(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    iovs: *const IoVec,
    num_iovs: usize,
    p_nbytes: *mut FileSize,
    write_fn: F,
) -> ErrNo {
    let bcfs = Box::leak(Box::from_raw(bcfs));
    let bufs = slice::from_raw_parts(iovs, num_iovs as usize)
        .iter()
        .map(|iov| IoSlice::new(slice::from_raw_parts(iov.buf, iov.len)))
        .collect::<Vec<_>>();
    match write_fn(bcfs, &mut *(*bc), &bufs) {
        Ok(nbytes) => {
            *p_nbytes = nbytes as FileSize;
            ErrNo::Success
        }
        Err(err) => err,
    }
}

#[no_mangle]
pub unsafe extern "C" fn write_vectored(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    fd: Fd,
    iovs: *const IoVec,
    num_iovs: usize,
    p_nbytes: *mut FileSize,
) -> ErrNo {
    do_write(bcfs, bc, iovs, num_iovs, p_nbytes, |bcfs, bc, bufs| {
        bcfs.write_vectored(bc, fd, bufs)
    })
}

#[no_mangle]
pub unsafe extern "C" fn pwrite_vectored(
    bcfs: *mut BCFS<Address>,
    bc: *const *mut dyn Blockchain<Address = Address>,
    fd: Fd,
    iovs: *const IoVec,
    num_iovs: usize,
    offset: FileSize,
    p_nbytes: *mut FileSize,
) -> ErrNo {
    do_write(bcfs, bc, iovs, num_iovs, p_nbytes, |bcfs, bc, bufs| {
        bcfs.pwrite_vectored(bc, fd, bufs, offset)
    })
}
