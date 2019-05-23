use std::cell::Cell;

use blockchain_traits::Address;
use wasi_types::{FdFlags, FileStat};

use crate::MultiAddress;

pub struct File<A: Address> {
    pub kind: FileKind<A>,
    pub offset: FileOffset,
    pub flags: FdFlags,
    pub(crate) metadata: Cell<Option<FileStat>>,
}

pub enum Filelike<A: Address> {
    File(File<A>),
    // Directory,
    // Socket,
    // Link,
}

pub enum FileKind<A: Address> {
    Stdin,
    Stdout,
    Stderr,
    Log,
    Regular { key: Vec<u8> },
    ServiceSock { addr: MultiAddress<A> },
    Bytecode { addr: MultiAddress<A> },
    Balance { addr: MultiAddress<A> },
}

#[derive(Clone, Copy)]
pub enum FileOffset {
    Stream, // sockets, for instance
    FromStart(u64),
    FromEnd(i64), // posix allows seeking past end of file
}

macro_rules! special_file_ctor {
    ($($fn:ident : $kind:ident),+) => {
        $(
        pub fn $fn() -> Self {
            Self {
                kind: FileKind::$kind,
                offset: FileOffset::FromStart(0),
                flags: FdFlags::APPEND | FdFlags::SYNC,
                metadata: Cell::new(None),
            }
        }
        )+
    }
}

impl<A: Address> File<A> {
    pub const LOG_DESCRIPTOR: u32 = 3;

    special_file_ctor!(stdin: Stdin, stdout: Stdout, stderr: Stderr, log: Log);

    pub fn is_special(&self) -> bool {
        match self.kind {
            FileKind::Stdin | FileKind::Stdout | FileKind::Stderr | FileKind::Log => true,
            _ => false,
        }
    }
}
