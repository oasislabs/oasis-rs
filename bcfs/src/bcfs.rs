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
    file::{File, FileCache, FileKind, CHAIN_DIR_FILENO, HOME_DIR_FILENO},
    Result,
};

pub struct BCFS<A: Address, M: AccountMeta> {
    files: Vec<Option<File<A>>>,
    home_addr: A,
    _account_meta: std::marker::PhantomData<M>,
}

impl<A: Address, M: AccountMeta> BCFS<A, M> {
    /// Creates a new ptx FS with a backing `ptx` and hex stringified
    /// owner address.
    pub fn new<S: AsRef<str>>(
        home_addr: A,
        // ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        blockchain_name: S,
    ) -> Self {
        Self {
            files: File::defaults(blockchain_name.as_ref()),
            home_addr,
            _account_meta: std::marker::PhantomData,
        }
    }

    pub fn prestat(
        &mut self,
        _ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<&Path> {
        match &self.file(fd)?.kind {
            FileKind::Directory { path } => Ok(path),
            _ => Err(ErrNo::BadF),
        }
    }

    pub fn open(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        curdir: Fd,
        path: &Path,
        open_flags: OpenFlags,
        fd_flags: FdFlags,
    ) -> Result<Fd> {
        if open_flags.contains(OpenFlags::DIRECTORY) {
            // The virutal filesystem does not yet allow opening directories.
            return Err(ErrNo::NotSup);
        }

        match &self.file(curdir)?.kind {
            FileKind::Directory { .. } => (),
            _ => return Err(ErrNo::BadF),
        };

        let mut file_exists = true;
        let file_kind = match self.canonicalize_path(curdir, path)? {
            (None, path) if path == Path::new("log") => FileKind::Log,
            (Some(addr), path) if path == Path::new("balance") => FileKind::Balance { addr },
            (Some(addr), path) if path == Path::new("bytecode") => FileKind::Bytecode { addr },
            (Some(addr), path) if addr == self.home_addr => {
                let key = Self::key_for_path(&path)?;
                file_exists = ptx.state().contains(&key);
                if file_exists && open_flags.contains(OpenFlags::EXCL) {
                    return Err(ErrNo::Exist);
                } else if !file_exists && !open_flags.contains(OpenFlags::CREATE) {
                    return Err(ErrNo::NoEnt);
                } else if !file_exists {
                    ptx.state_mut().set(&key, &[]);
                    // ^ This must be done eagerly to match POSIX which immediately creates the file.
                }
                FileKind::Regular { key }
            }
            _ => return Err(ErrNo::NoEnt),
        };

        if file_kind.is_blockchain_intrinsic() {
            if open_flags.intersects(OpenFlags::CREATE | OpenFlags::EXCL) {
                return Err(ErrNo::Exist);
            }
            if open_flags.intersects(OpenFlags::TRUNC | OpenFlags::DIRECTORY)
                || (file_kind.is_log() && !fd_flags.contains(FdFlags::APPEND))
                || (!file_kind.is_log() && fd_flags.contains(FdFlags::APPEND))
            {
                return Err(ErrNo::Inval);
            }
        }

        let fd = self.alloc_fd()?;
        self.files.push(Some(File {
            kind: file_kind,
            flags: fd_flags,
            metadata: Cell::new(if file_exists {
                None
            } else {
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
        }));
        Ok(fd)
    }

    pub fn flush(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<()> {
        self.do_flush(ptx, self.file(fd)?);
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

    /// Removes the file at `path` and returns the number of bytes previously in the file.
    pub fn unlink(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        curdir: Fd,
        path: &Path,
    ) -> Result<u64> {
        let curdir_fileno = u32::from(curdir);
        if curdir_fileno != HOME_DIR_FILENO {
            return Err(ErrNo::Access);
        }

        let (addr, path) = self.canonicalize_path(curdir, path)?;
        match addr {
            Some(addr) if addr == self.home_addr => (),
            _ => return Err(ErrNo::Access),
        }

        if path == Path::new("balance") || path == Path::new("bytecode") {
            return Err(ErrNo::Access);
        }

        let key = Self::key_for_path(&path)?;
        let state = ptx.state_mut();
        let prev_len = state.get(&key).unwrap_or_default().len() as u64;
        state.remove(&key);
        Ok(prev_len)
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

    pub fn sync(&mut self, ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>) {
        // flush stdout and stderr
        for file in self.files[1..=3].iter() {
            if let Some(file) = file {
                self.do_flush(ptx, file);
            }
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
    fn canonicalize_path(&self, curdir: Fd, path: &Path) -> Result<(Option<A>, PathBuf)> {
        use std::path::Component;

        if path.has_root() {
            return Err(ErrNo::NoEnt); // WASI paths must be releative to a preopened dir.
        }

        let curdir_fileno = u32::from(curdir);

        let mut canon_path = PathBuf::new();

        let mut comps = path
            .components()
            .skip_while(|comp| *comp == Component::CurDir)
            .peekable();

        let addr = if curdir_fileno == CHAIN_DIR_FILENO {
            match comps.peek() {
                Some(Component::Normal(maybe_addr)) => match maybe_addr.to_str().map(A::from_str) {
                    Some(Ok(addr)) => {
                        comps.next();
                        Some(addr)
                    }
                    _ => None,
                },
                Some(Component::Prefix(_)) | Some(Component::RootDir) => return Err(ErrNo::NoEnt),
                _ => None,
            }
        } else {
            Some(self.home_addr)
        };

        let mut has_path = false;
        for comp in comps {
            match comp {
                Component::Prefix(_) | Component::RootDir => return Err(ErrNo::NoEnt),
                Component::CurDir => (),
                Component::ParentDir => {
                    if !canon_path.pop() {
                        return Err(ErrNo::NoEnt);
                    }
                }
                Component::Normal(c) => {
                    has_path |= !c.is_empty();
                    canon_path.push(c);
                }
            }
        }

        if has_path {
            Ok((addr, canon_path))
        } else {
            Err(ErrNo::Inval)
        }
    }

    fn has_fd(&self, fd: Fd) -> bool {
        match self.files.get(fd_usize(fd)) {
            Some(Some(_)) => true,
            _ => false,
        }
    }

    fn file(&self, fd: Fd) -> Result<&File<A>> {
        match self.files.get(fd_usize(fd)) {
            Some(Some(file)) => Ok(file),
            _ => Err(ErrNo::BadF),
        }
    }

    fn file_mut(&mut self, fd: Fd) -> Result<&mut File<A>> {
        match self
            .files
            .get_mut(usize::try_from(u64::from(fd)).map_err(|_| ErrNo::BadF)?)
        {
            Some(Some(file)) => Ok(file),
            _ => Err(ErrNo::BadF),
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

    fn key_for_path(path: &Path) -> Result<Vec<u8>> {
        path.to_str()
            .ok_or(ErrNo::Inval)
            .map(|s| s.as_bytes().to_vec())
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
                    FileKind::Bytecode { addr } => match ptx.code_at(&addr) {
                        Some(code) => code.to_vec(),
                        None => return Err(ErrNo::NoEnt),
                    },
                    FileKind::Balance { addr } => match ptx.account_meta_at(&addr) {
                        Some(meta) => meta.balance().to_le_bytes().to_vec(),
                        None => return Err(ErrNo::NoEnt),
                    },
                    FileKind::Regular { key } => match ptx.state().get(&key) {
                        Some(val) => val.to_vec(),
                        None => return Err(ErrNo::NoEnt),
                    },
                    FileKind::Stdout | FileKind::Stderr | FileKind::Log => Vec::new(),
                    FileKind::Directory { .. } => return Err(ErrNo::Fault),
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

    fn do_flush(
        &self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        file: &File<A>,
    ) {
        if !file.dirty.get() {
            return;
        }
        let maybe_cursor = file.buf.borrow();
        let buf = match &*maybe_cursor {
            FileCache::Present(cursor) => cursor.get_ref(),
            FileCache::Absent(_) => return,
        };
        match &file.kind {
            FileKind::Stdin | FileKind::Bytecode { .. } | FileKind::Balance { .. } => (),
            FileKind::Stdout => ptx.ret(buf),
            FileKind::Stderr => ptx.err(buf),
            FileKind::Log => {
                if let Some((topics, data)) = Self::parse_log(buf) {
                    ptx.emit(&topics, data);
                }
            }
            FileKind::Regular { key } => {
                ptx.state_mut().set(&key, &buf);
                for f in self.files[(HOME_DIR_FILENO as usize + 1)..].iter() {
                    match f {
                        Some(f) => match f {
                            File {
                                kind: FileKind::Regular { key: f_key },
                                ..
                            } if key == f_key && f as *const File<A> != file as *const File<A> => {
                                let mut f_buf = f.buf.borrow_mut();
                                let mut cursor = Cursor::new(buf.clone());
                                cursor
                                    .seek(match &*f_buf {
                                        FileCache::Absent(seek_from) => *seek_from,
                                        FileCache::Present(cursor) => {
                                            SeekFrom::Start(cursor.position())
                                        }
                                    })
                                    .ok(); // deal with the error when the file is actually read
                                *f_buf = FileCache::Present(cursor);
                                f.metadata.replace(None);
                            }
                            _ => (),
                        },
                        None => (),
                    }
                }
                file.dirty.set(false);
            }
            FileKind::Directory { .. } => (),
        }
    }

    fn parse_log(buf: &[u8]) -> Option<(Vec<&[u8]>, &[u8])> {
        use nom::{length_count, length_data, named, number::complete::le_u32, tuple};
        named!(parser<&[u8], (Vec<&[u8]>, &[u8])>, tuple!(
            length_count!(le_u32, length_data!(le_u32)), length_data!(le_u32)
        ));
        match parser(buf) {
            Ok(([], result)) => Some(result),
            _ => None,
        }
    }
}
