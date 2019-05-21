enum Filelike {
    File(File),
    // Directory,
    // Socket,
    // Link,
}

struct File {
    kind: FileKind,
    offset: FileOffset,
    flags: FdFlags,
    metadata: Cell<Option<FileStat>>,
}

enum FileKind {
    Stdin,
    Stdout,
    Stderr,
    Log,
    Regular { key: Vec<u8> },
    Bytecode { addr: Address },
}

#[derive(Clone, Copy)]
enum FileOffset {
    FromStart(u64),
    FromEnd(i64), // posix allows seeking past end of file
}

macro_rules! special_file_ctor {
    ($($fn:ident : $kind:ident),+) => {
        $(
        fn $fn() -> Self {
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

impl File {
    const LOG_DESCRIPTOR: u32 = 3;

    special_file_ctor!(stdin: Stdin, stdout: Stdout, stderr: Stderr, log: Log);

    fn is_special(&self) -> bool {
        match self.kind {
            FileKind::Stdin | FileKind::Stdout | FileKind::Stderr | FileKind::Log => true,
            _ => false,
        }
    }
}
