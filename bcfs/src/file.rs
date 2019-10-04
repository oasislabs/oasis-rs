use std::{
    cell::{Cell, RefCell},
    io::{Cursor, SeekFrom},
    path::PathBuf,
};

use oasis_types::Address;
use wasi_types::{FdFlags, FileStat};

pub struct File {
    pub kind: FileKind,

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

pub enum FileKind {
    Stdin,
    Stdout,
    Stderr,
    Log,
    Temporary,
    Regular { key: Vec<u8> },
    Balance { addr: Address },
    Bytecode { addr: Address },
    Directory { path: PathBuf },
}

impl FileKind {
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
                // Generate each of stdin, stdout, and stderr.
                $(
                    Some(Self {
                        kind: FileKind::$kind,
                        flags: FdFlags::APPEND | FdFlags::SYNC,
                        metadata: Cell::new(None),
                        buf: RefCell::new(FileCache::Absent(SeekFrom::Start(0))),
                        dirty: Cell::new(false),
                    })
                ),+,

                // This fd is the capability to the chain dir (e.g., `/opt/oasis/`) which
                // contains the `log` file, among other things.
                // This capability (and all other directory caps) will be discovered when
                // the WASI libc calls `fd_prestat_get` during the `_start` function.
                Some(Self {
                    kind: FileKind::Directory { path: chain_dir },
                    flags: FdFlags::SYNC,
                    metadata: Cell::new(None),
                    buf: RefCell::new(FileCache::Absent(SeekFrom::Start(0))),
                    dirty: Cell::new(false),
                }),

                // This fd is the capability to the service's home directory.
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

impl File {
    special_file_ctor!(Stdin, Stdout, Stderr);
}

pub const CHAIN_DIR_FILENO: u32 = 3;
pub const HOME_DIR_FILENO: u32 = 4;
