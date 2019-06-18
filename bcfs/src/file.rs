use std::{
    cell::{Cell, RefCell},
    io::{Cursor, SeekFrom},
};

use blockchain_traits::Address;
use wasi_types::{FdFlags, FileStat};

use crate::AnyAddress;

pub struct File<A: Address> {
    pub kind: FileKind<A>,

    pub flags: FdFlags,

    /// File metadata cache.
    pub metadata: Cell<Option<FileStat>>,

    /// File contents cache.
    pub buf: RefCell<FileCache>,

    /// Whether the file has data that needs to be written back to the trie.
    pub dirty: Cell<bool>,
}

pub enum FileCache {
    Absent(SeekFrom),
    Present(Cursor<Vec<u8>>),
}

pub enum Filelike<A: Address> {
    File(File<A>),
}

pub enum FileKind<A: Address> {
    Stdin,
    Stdout,
    Stderr,
    Log,
    Regular { key: Vec<u8> },
    Bytecode { addr: AnyAddress<A> },
    Balance { addr: AnyAddress<A> },
}

macro_rules! special_file_ctor {
    ($($fn:ident : $kind:ident),+) => {
        $(
        pub fn $fn() -> Self {
            Self {
                kind: FileKind::$kind,
                flags: FdFlags::APPEND | FdFlags::SYNC,
                metadata: Cell::new(None),
                buf: RefCell::new(FileCache::Absent(SeekFrom::Start(0))),
                dirty: Cell::new(false),
            }
        }
        )+
    }
}

impl<A: Address> File<A> {
    pub const LOG_DESCRIPTOR: u32 = 3;

    special_file_ctor!(stdin: Stdin, stdout: Stdout, stderr: Stderr, log: Log);
}
