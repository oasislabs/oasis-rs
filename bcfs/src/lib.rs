#![feature(bind_by_move_pattern_guards)]

#[cfg(feature = "ffi")]
pub mod ffi;

use std::{
    cell::{Cell, RefCell},
    convert::TryFrom,
    io::{IoSlice, IoSliceMut, Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

use blockchain_traits::{BlockchainIntrinsics, KVStore};
use oasis_types::Address;
use wasi_types::{
    ErrNo, Fd, FdFlags, FdStat, FileDelta, FileSize, FileStat, FileType, Inode, OpenFlags, Rights,
    Whence,
};

type Result<T> = std::result::Result<T, ErrNo>;

include!("bcfs.rs");
include!("file.rs");

#[cfg(test)]
mod tests;
