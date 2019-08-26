use std::{
    cell::{Cell, RefCell},
    io::{Cursor, SeekFrom},
    path::PathBuf,
};

use blockchain_traits::Address;
use wasi_types::{FdFlags, FileStat};

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

pub enum FileKind<A: Address> {
    Stdin,
    Stdout,
    Stderr,
    Log,
    Temporary,
    Regular { key: Vec<u8> },
    Balance { addr: A },
    Bytecode { addr: A },
    Directory { path: PathBuf },
}

impl<A: Address> FileKind<A> {
    pub fn is_log(&self) -> bool {
        match self {
            FileKind::Log => true,
            _ => false,
        }
    }

    pub fn is_blockchain_intrinsic(&self) -> bool {
        match self {
            FileKind::Log | FileKind::Balance { .. } | FileKind::Bytecode { .. } => true,
            _ => false,
        }
    }
}

macro_rules! special_file_ctor {
    ($($kind:ident),+) => {
        pub fn defaults(blockchain_name: &str) -> Vec<Option<Self>> {
            let mut chain_dir = PathBuf::from("/opt");
            chain_dir.push(blockchain_name);

            vec![
                $(Some(Self {
                    kind: FileKind::$kind,
                    flags: FdFlags::APPEND | FdFlags::SYNC,
                    metadata: Cell::new(None),
                    buf: RefCell::new(FileCache::Absent(SeekFrom::Start(0))),
                    dirty: Cell::new(false),
                })),+,
                Some(Self {
                    kind: FileKind::Directory { path: chain_dir },
                    flags: FdFlags::SYNC,
                    metadata: Cell::new(None),
                    buf: RefCell::new(FileCache::Absent(SeekFrom::Start(0))),
                    dirty: Cell::new(false),
                }),
                Some(Self {
                    kind: FileKind::Directory { path: PathBuf::from(".") },
                    flags: FdFlags::SYNC,
                    metadata: Cell::new(None),
                    buf: RefCell::new(FileCache::Absent(SeekFrom::Start(0))),
                    dirty: Cell::new(false),
                }),
            ]
        }
    }
}

impl<A: Address> File<A> {
    special_file_ctor!(Stdin, Stdout, Stderr);
}

pub const CHAIN_DIR_FILENO: u32 = 3;
pub const HOME_DIR_FILENO: u32 = 4;
