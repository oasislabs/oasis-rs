use std::{
    cell::{Cell, RefCell},
    convert::TryFrom,
    io::{Cursor, IoSlice, IoSliceMut, Read, Seek as _, SeekFrom, Write},
    path::{Path, PathBuf},
};

use blockchain_traits::{AccountMeta, Address, PendingTransaction};
use wasi_types::{
    ErrNo, Fd, FdFlags, FdStat, FileDelta, FileSize, FileStat, FileType, OpenFlags, Rights, Whence,
};

use crate::{
    file::{File, FileCache, FileKind, Filelike},
    AnyAddress, Result,
};

pub struct BCFS<A: Address, M: AccountMeta> {
    files: Vec<Option<Filelike<A>>>,
    blockchain_name: String,
    home_dir: PathBuf,
    _account_meta: std::marker::PhantomData<M>,
}

impl<A: Address, M: AccountMeta> BCFS<A, M> {
    /// Creates a new ptx FS with a backing `ptx` and hex stringified
    /// owner address.
    pub fn new<S: AsRef<str>>(
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        blockchain_name: S,
    ) -> Self {
        let mut home_dir = PathBuf::from("/opt");
        home_dir.push(blockchain_name.as_ref());
        home_dir.push(ptx.address().path_repr());

        Self {
            files: vec![
                Some(Filelike::File(File::stdin())),
                Some(Filelike::File(File::stdout())),
                Some(Filelike::File(File::stderr())),
                Some(Filelike::File(File::log())),
            ],
            blockchain_name: blockchain_name.as_ref().to_string(),
            home_dir,
            _account_meta: std::marker::PhantomData,
        }
    }

    pub fn open(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        curdir: Option<Fd>,
        path: &Path,
        open_flags: OpenFlags,
        fd_flags: FdFlags,
    ) -> Result<Fd> {
        if open_flags.contains(OpenFlags::DIRECTORY) | curdir.is_some() {
            // The virutal filesystem does not yet support directories.
            return Err(ErrNo::NotSup);
        }

        let path = self.canonicalize_path(path)?;

        if path == Path::new("/log") {
            if open_flags.intersects(OpenFlags::CREATE | OpenFlags::EXCL) {
                return Err(ErrNo::Exist);
            }
            if !fd_flags.contains(FdFlags::APPEND)
                || open_flags.intersects(OpenFlags::TRUNC | OpenFlags::DIRECTORY)
            {
                return Err(ErrNo::Inval);
            }
            return Ok(File::<A>::LOG_DESCRIPTOR.into());
        }

        if let Some(svc_file_kind) = self.is_service_path(ptx, &path) {
            if open_flags.intersects(OpenFlags::CREATE | OpenFlags::EXCL) {
                return Err(ErrNo::Exist);
            }
            if fd_flags.contains(FdFlags::APPEND)
                || open_flags.intersects(OpenFlags::TRUNC | OpenFlags::DIRECTORY)
            {
                return Err(ErrNo::Inval);
            }
            let fd = self.alloc_fd()?;
            self.files.push(Some(Filelike::File(File {
                kind: svc_file_kind,
                flags: fd_flags,
                metadata: Cell::new(None),
                buf: RefCell::new(FileCache::Absent(SeekFrom::Start(0))),
                dirty: Cell::new(false),
            })));
            return Ok(fd);
        }

        if !path.starts_with(&self.home_dir) {
            // there are no other special files and those outside of the home directory are
            // (currently) defined as not existing. This will change once services can pass
            // capabilities to their storage to other services.
            return Err(ErrNo::NoEnt);
        }

        let key = Self::key_for_path(&path)?;
        let file_exists = ptx.state().contains(&key);

        if file_exists && open_flags.contains(OpenFlags::EXCL) {
            return Err(ErrNo::Exist);
        } else if !file_exists && !open_flags.contains(OpenFlags::CREATE) {
            return Err(ErrNo::NoEnt);
        }

        let fd = self.alloc_fd()?;
        self.files.push(Some(Filelike::File(File {
            kind: FileKind::Regular { key: key.to_vec() },
            flags: fd_flags,
            metadata: Cell::new(if file_exists {
                None
            } else {
                ptx.state_mut().set(key, &Vec::new());
                // ^ This must be done eagerly to match POSIX which immediately creates the file.
                Some(FileStat {
                    device: 0u64.into(),
                    inode: 0u32.into(),
                    file_type: FileType::RegularFile,
                    num_links: 0,
                    file_size: 0,
                    atime: 0u64.into(),
                    mtime: 0u64.into(),
                    ctime: 0u64.into(),
                })
            }),
            buf: RefCell::new(if file_exists {
                FileCache::Absent(SeekFrom::Start(0))
            } else {
                FileCache::Present(Cursor::new(Vec::new()))
            }),
            dirty: Cell::new(false),
        })));
        Ok(fd)
    }

    pub fn flush(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<()> {
        let file = self.file(fd)?;
        if !file.dirty.get() {
            return Ok(());
        }
        let maybe_cursor = file.buf.borrow();
        let buf = match &*maybe_cursor {
            FileCache::Present(cursor) => cursor.get_ref(),
            FileCache::Absent(_) => return Ok(()),
        };
        match &file.kind {
            FileKind::Stdin | FileKind::Bytecode { .. } | FileKind::Balance { .. } => (),
            FileKind::Stdout => ptx.ret(buf),
            FileKind::Stderr => ptx.err(buf),
            FileKind::Log => {
                // NOTE: topics must be written as a block of TOPIC_SIZE * MAX_TOPICS bytes.
                // Space for unused topics should be set to zero.
                const TOPIC_SIZE: usize = 32; // 256 bits
                const MAX_TOPICS: usize = 4;
                let (cat_topics, data) = buf.split_at(TOPIC_SIZE * MAX_TOPICS);
                let topics = cat_topics.chunks_exact(TOPIC_SIZE).collect::<Vec<&[u8]>>();
                ptx.emit(&topics, &data);
            }
            FileKind::Regular { key } => ptx.state_mut().set(&key, &buf),
        }
        Ok(())
    }

    pub fn close(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<()> {
        self.flush(ptx, fd)?;
        match self.files.get_mut(u32::from(fd) as usize) {
            Some(f) if f.is_some() => {
                *f = None;
                Ok(())
            }
            _ => Err(ErrNo::BadF),
        }
    }

    pub fn seek(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        offset: FileDelta,
        whence: Whence,
    ) -> Result<FileSize> {
        let file = self.file_mut(fd)?;

        let mut buf = file.buf.borrow_mut();

        if Whence::End == whence
            || match &*buf {
                FileCache::Absent(SeekFrom::End(_)) => true,
                _ => false,
            }
        {
            Self::populate_file(ptx, &file, &mut *buf)?;
        }

        match &mut *buf {
            FileCache::Present(ref mut cursor) => {
                Ok(cursor.seek(seekfrom_from_offset_whence(offset, whence)?)?)
            }
            FileCache::Absent(ref mut seek) => match whence {
                Whence::End => unreachable!("file was just populated"),
                Whence::Start => {
                    *seek = seekfrom_from_offset_whence(offset, whence)?;
                    Ok(offset as u64)
                }
                Whence::Current => match seek {
                    SeekFrom::Start(cur_offset) => {
                        let new_offset = Self::checked_offset(*cur_offset, offset)?;
                        *seek = SeekFrom::Start(new_offset);
                        Ok(new_offset as u64)
                    }
                    _ => unreachable!("handled above"),
                },
            },
        }
    }

    pub fn fdstat(
        &self,
        _ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<FdStat> {
        let file = self.file(fd)?;
        Ok(FdStat {
            file_type: FileType::RegularFile,
            flags: file.flags,
            rights_base: Rights::all(),
            rights_inheriting: Rights::all(),
        })
    }

    pub fn filestat(
        &self,
        ptx: &dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<FileStat> {
        let file = self.file(fd)?;
        Self::populate_file(ptx, file, &mut *file.buf.borrow_mut())
    }

    pub fn tell(
        &self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<FileSize> {
        let file = self.file(fd)?;
        let mut buf = file.buf.borrow_mut();
        if let FileCache::Absent(SeekFrom::End(_)) = &*buf {
            Self::populate_file(ptx, &file, &mut *buf)?;
        }
        Ok(match &mut *buf {
            FileCache::Present(cursor) => cursor.position(),
            FileCache::Absent(ref mut seekfrom) => match seekfrom {
                SeekFrom::Start(offset) => *offset,
                SeekFrom::End(_) => unreachable!("checked above"),
                SeekFrom::Current(_) => unreachable!(),
            },
        })
    }

    pub fn read_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &mut [IoSliceMut],
    ) -> Result<usize> {
        self.do_pread_vectored(ptx, fd, bufs, None)
    }

    pub fn pread_vectored(
        &self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &mut [IoSliceMut],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pread_vectored(ptx, fd, bufs, Some(SeekFrom::Start(offset)))
    }

    pub fn write_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &[IoSlice],
    ) -> Result<usize> {
        self.do_pwrite_vectored(ptx, fd, bufs, None)
    }

    pub fn pwrite_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &[IoSlice],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pwrite_vectored(ptx, fd, bufs, Some(SeekFrom::Start(offset)))
    }

    pub fn renumber(
        &mut self,
        _ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        new_fd: Fd,
    ) -> Result<()> {
        if self.has_fd(fd) && self.has_fd(new_fd) {
            self.files.swap(fd_usize(fd), fd_usize(new_fd));
            self.files[fd_usize(fd)] = None;
            Ok(())
        } else {
            Err(ErrNo::BadF)
        }
    }
}

fn fd_usize(fd: Fd) -> usize {
    usize::try_from(u32::from(fd)).unwrap() // can't fail because usize is at least 32 bits
}

fn seekfrom_from_offset_whence(offset: FileDelta, whence: Whence) -> Result<SeekFrom> {
    Ok(match whence {
        Whence::Current => SeekFrom::Current(offset),
        Whence::Start => SeekFrom::Start(if offset < 0 {
            return Err(ErrNo::Inval);
        } else {
            offset as u64
        }),
        Whence::End => SeekFrom::End(offset),
    })
}

impl<A: Address, M: AccountMeta> BCFS<A, M> {
    fn canonicalize_path(&self, path: &Path) -> Result<PathBuf> {
        use std::path::Component;
        let mut canon_path = if path.has_root() {
            PathBuf::new()
        } else {
            self.home_dir.clone()
        };
        for component in path.components() {
            match component {
                Component::Prefix(_) => return Err(ErrNo::NoEnt),
                Component::RootDir => canon_path.push("/"),
                Component::CurDir => (),
                Component::ParentDir => {
                    if !canon_path.pop() {
                        return Err(ErrNo::NoEnt);
                    }
                }
                Component::Normal(c) => canon_path.push(c),
            }
        }
        Ok(canon_path)
    }

    fn has_fd(&self, fd: Fd) -> bool {
        match self.files.get(fd_usize(fd)) {
            Some(Some(_)) => true,
            _ => false,
        }
    }

    fn file(&self, fd: Fd) -> Result<&File<A>> {
        match self.files.get(fd_usize(fd)) {
            Some(Some(Filelike::File(file))) => Ok(file),
            _ => Err(ErrNo::BadF),
        }
    }

    fn file_mut(&mut self, fd: Fd) -> Result<&mut File<A>> {
        match self
            .files
            .get_mut(usize::try_from(u64::from(fd)).map_err(|_| ErrNo::BadF)?)
        {
            Some(Some(Filelike::File(file))) => Ok(file),
            _ => Err(ErrNo::BadF),
        }
    }

    fn is_service_path(
        &self,
        ptx: &dyn PendingTransaction<Address = A, AccountMeta = M>,
        path: &Path,
    ) -> Option<FileKind<A>> {
        use std::path::Component;

        if path == Path::new("code") {
            return Some(FileKind::Bytecode {
                addr: AnyAddress::Native(*ptx.address()),
            });
        } else if path == Path::new("balance") {
            return Some(FileKind::Balance {
                addr: AnyAddress::Native(*ptx.address()),
            });
        }

        let mut comps = path.components();

        match comps.next() {
            Some(Component::RootDir) => (),
            _ => return None,
        }

        match comps.next() {
            Some(Component::Normal(c)) if c == "opt" => (),
            _ => return None,
        }

        match comps.next() {
            Some(Component::Normal(c)) if *c == self.blockchain_name.as_ref() => (),
            _ => return None,
        }

        let addr = match comps
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .map(A::from_str)
        {
            Some(Ok(addr)) => AnyAddress::Native(addr),
            _ => return None,
        };

        let svc_file_kind = match comps.next() {
            Some(Component::Normal(c)) if c == "code" => FileKind::Bytecode { addr },
            Some(Component::Normal(c)) if c == "balance" => FileKind::Balance { addr },
            _ => return None,
        };

        match comps.next() {
            None => Some(svc_file_kind),
            _ => None,
        }
    }

    fn alloc_fd(&self) -> Result<Fd> {
        if self.files.len() >= u32::max_value() as usize {
            return Err(ErrNo::NFile); // TODO(#82)
        }
        Ok((self.files.len() as u32).into())
    }

    fn checked_offset(base: u64, offset: i64) -> Result<u64> {
        if offset >= 0 {
            base.checked_add(offset as u64)
        } else {
            base.checked_sub(-offset as u64)
        }
        .ok_or(ErrNo::Inval)
    }

    fn key_for_path(path: &Path) -> Result<&[u8]> {
        path.to_str().ok_or(ErrNo::Inval).map(str::as_bytes)
    }

    fn populate_file(
        ptx: &dyn PendingTransaction<Address = A, AccountMeta = M>,
        file: &File<A>,
        cache: &mut FileCache,
    ) -> Result<FileStat> {
        let file_size = match cache {
            FileCache::Present(cursor) => cursor.get_ref().len(),
            FileCache::Absent(offset) => {
                let bytes = match &file.kind {
                    FileKind::Stdin => ptx.input().to_vec(),
                    FileKind::Bytecode {
                        addr: crate::AnyAddress::Native(addr),
                    } => match ptx.code_at(&addr) {
                        Some(code) => code.to_vec(),
                        None => return Err(ErrNo::NoEnt),
                    },
                    FileKind::Balance {
                        addr: crate::AnyAddress::Native(addr),
                    } => match ptx.account_meta_at(&addr) {
                        Some(meta) => meta.balance().to_le_bytes().to_vec(),
                        None => return Err(ErrNo::NoEnt),
                    },
                    FileKind::Regular { key } => match ptx.state().get(&key) {
                        Some(val) => val.to_vec(),
                        None => return Err(ErrNo::NoEnt),
                    },
                    FileKind::Stdout | FileKind::Stderr | FileKind::Log => Vec::new(),
                    FileKind::Bytecode {
                        addr: crate::AnyAddress::Foreign(_),
                    }
                    | FileKind::Balance {
                        addr: crate::AnyAddress::Foreign(_),
                    } => return Err(ErrNo::Fault),
                };
                let file_size = bytes.len();
                let mut cursor = Cursor::new(bytes);
                cursor.seek(*offset)?;
                *cache = FileCache::Present(cursor);
                file_size
            }
        } as u64;
        match file.metadata.get() {
            Some(meta) => Ok(meta),
            None => {
                let meta = FileStat {
                    device: 0u64.into(),
                    inode: 0u64.into(), // TODO(#80)
                    file_type: FileType::RegularFile,
                    num_links: 0,
                    file_size,
                    atime: 0u64.into(), // TODO(#81)
                    mtime: 0u64.into(),
                    ctime: 0u64.into(),
                };
                file.metadata.set(Some(meta));
                Ok(meta)
            }
        }
    }

    /// Returns (bytes_read, new_offset).
    fn do_pread_vectored(
        &self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &mut [IoSliceMut],
        offset: Option<SeekFrom>,
    ) -> Result<usize> {
        let file = self.file(fd)?;
        match file.kind {
            FileKind::Stdout | FileKind::Stderr { .. } | FileKind::Log { .. } => {
                return Err(ErrNo::Inval)
            }
            _ => (),
        };

        let mut buf = file.buf.borrow_mut();
        Self::populate_file(ptx, &file, &mut *buf)?;

        let cursor = match &mut *buf {
            FileCache::Present(ref mut cursor) => cursor,
            FileCache::Absent(_) => unreachable!("file was just populated"),
        };

        match offset {
            Some(offset) => {
                let orig_pos = cursor.position();
                cursor.seek(offset)?;
                let nbytes = cursor.read_vectored(bufs)?;
                cursor.set_position(orig_pos);
                Ok(nbytes)
            }
            None => Ok(cursor.read_vectored(bufs)?),
        }
    }

    /// Returns (bytes_written, new_offset).
    fn do_pwrite_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &[IoSlice],
        offset: Option<SeekFrom>,
    ) -> Result<usize> {
        let file = self.file(fd)?;
        match file.kind {
            FileKind::Stdin | FileKind::Bytecode { .. } | FileKind::Balance { .. } => {
                return Err(ErrNo::Inval)
            }
            _ => (),
        };

        let mut buf = file.buf.borrow_mut();
        Self::populate_file(ptx, &file, &mut *buf)?;

        let cursor = match &mut *buf {
            FileCache::Present(ref mut cursor) => cursor,
            FileCache::Absent(_) => unreachable!("file was just populated"),
        };

        let nbytes = match offset {
            Some(offset) => {
                let orig_pos = cursor.position();
                cursor.seek(offset)?;
                let nbytes = cursor.write_vectored(bufs)?;
                cursor.set_position(orig_pos);
                nbytes
            }
            None => cursor.write_vectored(bufs)?,
        };
        if nbytes > 0 {
            file.dirty.replace(true);
        }
        Ok(nbytes)
    }
}
