use std::{
    cell::{Cell, RefCell},
    convert::TryFrom as _,
    io::{Cursor, IoSlice, IoSliceMut, Read as _, Seek as _, SeekFrom, Write as _},
    path::{Path, PathBuf},
    str::FromStr as _,
};

use blockchain_traits::PendingTransaction;
use oasis_types::Address;
use wasi_types::{
    ErrNo, Fd, FdFlags, FdStat, FileDelta, FileSize, FileStat, FileType, OpenFlags, Rights, Whence,
};

use crate::{
    file::{File, FileCache, FileKind, CHAIN_DIR_FILENO, HOME_DIR_FILENO},
    Result,
};

pub struct BCFS {
    files: Vec<Option<File>>,
    home_addr: Address,
}

impl BCFS {
    /// Creates a new ptx FS with a backing `ptx` and hex stringified
    /// owner address.
    pub fn new<S: AsRef<str>>(
        home_addr: Address,
        // ptx: &mut dyn PendingTransaction< AccountMeta = M>,
        blockchain_name: S,
    ) -> Self {
        Self {
            files: File::defaults(blockchain_name.as_ref()),
            home_addr,
        }
    }

    pub fn prestat(&mut self, _ptx: &mut dyn PendingTransaction, fd: Fd) -> Result<&Path> {
        match &self.file(fd)?.kind {
            FileKind::Directory { path } => Ok(path),
            _ => Err(ErrNo::BadF),
        }
    }

    pub fn open(
        &mut self,
        ptx: &mut dyn PendingTransaction,
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
                Some(Self::default_filestat())
            }),
            buf: RefCell::new(if !file_exists || open_flags.contains(OpenFlags::TRUNC) {
                FileCache::Present(Cursor::new(Vec::new()))
            } else {
                FileCache::Absent(if fd_flags.contains(FdFlags::APPEND) {
                    SeekFrom::End(0)
                } else {
                    SeekFrom::Start(0)
                })
            }),
            dirty: Cell::new(false),
        }));
        Ok(fd)
    }

    pub fn tempfile(&mut self, _ptx: &mut dyn PendingTransaction) -> Result<Fd> {
        let fd = self.alloc_fd()?;
        self.files.push(Some(File {
            kind: FileKind::Temporary,
            flags: FdFlags::empty(),
            metadata: Cell::new(Some(Self::default_filestat())),
            buf: RefCell::new(FileCache::Present(Cursor::new(Vec::new()))),
            dirty: Cell::new(false),
        }));
        Ok(fd)
    }

    pub fn flush(&mut self, ptx: &mut dyn PendingTransaction, fd: Fd) -> Result<()> {
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
            FileKind::Stdin
            | FileKind::Bytecode { .. }
            | FileKind::Balance { .. }
            | FileKind::Directory { .. }
            | FileKind::Temporary => (),
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
                    if let Some(File {
                        kind: FileKind::Regular { key: f_key },
                        ..
                    }) = f
                    {
                        let f = f.as_ref().unwrap();
                        if key != f_key || f as *const File == file as *const File {
                            continue;
                        }
                        let mut f_buf = f.buf.borrow_mut();
                        let mut cursor = Cursor::new(buf.clone());
                        cursor
                            .seek(match &*f_buf {
                                FileCache::Absent(seek_from) => *seek_from,
                                FileCache::Present(cursor) => SeekFrom::Start(cursor.position()),
                            })
                            .ok(); // deal with the error when the file is actually read
                        *f_buf = FileCache::Present(cursor);
                        f.metadata.replace(None);
                    }
                }
                file.dirty.set(false);
            }
        }
        Ok(())
    }

    pub fn close(&mut self, ptx: &mut dyn PendingTransaction, fd: Fd) -> Result<()> {
        self.flush(ptx, fd)?;
        match self.files.get_mut(fd_usize(fd)) {
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
        ptx: &mut dyn PendingTransaction,
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
        ptx: &mut dyn PendingTransaction,
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

    pub fn fdstat(&self, _ptx: &mut dyn PendingTransaction, fd: Fd) -> Result<FdStat> {
        let file = self.file(fd)?;
        Ok(FdStat {
            file_type: match file.kind {
                FileKind::Directory { .. } => FileType::Directory,
                _ => FileType::RegularFile,
            },
            flags: file.flags,
            rights_base: Rights::all(),
            rights_inheriting: Rights::all(),
        })
    }

    pub fn filestat(&self, ptx: &dyn PendingTransaction, fd: Fd) -> Result<FileStat> {
        let file = self.file(fd)?;
        Self::populate_file(ptx, file, &mut *file.buf.borrow_mut())
    }

    pub fn tell(&self, ptx: &mut dyn PendingTransaction, fd: Fd) -> Result<FileSize> {
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
        ptx: &mut dyn PendingTransaction,
        fd: Fd,
        bufs: &mut [IoSliceMut],
    ) -> Result<usize> {
        self.do_pread_vectored(ptx, fd, bufs, None)
    }

    pub fn pread_vectored(
        &self,
        ptx: &mut dyn PendingTransaction,
        fd: Fd,
        bufs: &mut [IoSliceMut],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pread_vectored(ptx, fd, bufs, Some(SeekFrom::Start(offset)))
    }

    pub fn write_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction,
        fd: Fd,
        bufs: &[IoSlice],
    ) -> Result<usize> {
        self.do_pwrite_vectored(ptx, fd, bufs, None)
    }

    pub fn pwrite_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction,
        fd: Fd,
        bufs: &[IoSlice],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pwrite_vectored(ptx, fd, bufs, Some(SeekFrom::Start(offset)))
    }

    pub fn renumber(
        &mut self,
        _ptx: &mut dyn PendingTransaction,
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

impl BCFS {
    fn canonicalize_path(&self, curdir: Fd, path: &Path) -> Result<(Option<Address>, PathBuf)> {
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
                Some(Component::Normal(maybe_addr)) => {
                    match maybe_addr.to_str().map(Address::from_str) {
                        Some(Ok(addr)) => {
                            comps.next();
                            Some(addr)
                        }
                        _ => None,
                    }
                }
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

    fn file(&self, fd: Fd) -> Result<&File> {
        match self.files.get(fd_usize(fd)) {
            Some(Some(file)) => Ok(file),
            _ => Err(ErrNo::BadF),
        }
    }

    fn file_mut(&mut self, fd: Fd) -> Result<&mut File> {
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
        ptx: &dyn PendingTransaction,
        file: &File,
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
                        Some(meta) => meta.balance.to_le_bytes().to_vec(),
                        None => return Err(ErrNo::NoEnt),
                    },
                    FileKind::Regular { key } => match ptx.state().get(&key) {
                        Some(val) => val.to_vec(),
                        None => return Err(ErrNo::NoEnt),
                    },
                    FileKind::Stdout | FileKind::Stderr | FileKind::Log => Vec::new(),
                    FileKind::Directory { .. } | FileKind::Temporary => return Err(ErrNo::Fault),
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

    fn do_pread_vectored(
        &self,
        ptx: &mut dyn PendingTransaction,
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

    fn do_pwrite_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction,
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

    /// Parses a log buffer into (topics, data)
    /// Format:
    /// num_topics [topic_len [topic_data; topic_len]; num_topics] data_len [data; data_len]
    /// num_* are little-endian 32-bit integers.
    fn parse_log(buf: &[u8]) -> Option<(Vec<&[u8]>, &[u8])> {
        use nom::{complete, do_parse, length_count, length_data, named, number::complete::le_u32};
        named! {
            parser<(Vec<&[u8]>, &[u8])>,
            complete!(do_parse!(
                topics: length_count!(le_u32, length_data!(le_u32)) >>
                data:   length_data!(le_u32)                        >>
                (topics, data)
            ))
        };
        parser(buf).map(|result| result.1).ok()
    }

    fn default_filestat() -> FileStat {
        FileStat {
            device: 0u64.into(),
            inode: 0u32.into(),
            file_type: FileType::RegularFile,
            num_links: 0,
            file_size: 0,
            atime: 0u64.into(),
            mtime: 0u64.into(),
            ctime: 0u64.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log() {
        let topics = vec![b"hello", b"world"];
        let num_topics_bytes = (topics.len() as u32).to_le_bytes();
        let topic_lens: Vec<Vec<u8>> = topics
            .iter()
            .map(|t| (t.len() as u32).to_le_bytes().to_vec())
            .collect();
        let topics_bytes: Vec<u8> = num_topics_bytes
            .iter()
            .chain(
                topics
                    .iter()
                    .enumerate()
                    .flat_map(|(i, t)| topic_lens[i].iter().chain(t.iter())),
            )
            .copied()
            .collect();

        let data = b"I bid thee hello!";
        let data_len = (data.len() as u32).to_le_bytes();

        let log: Vec<u8> = std::iter::empty()
            .chain(topics_bytes.iter())
            .chain(data_len.iter())
            .chain(data.iter())
            .copied()
            .collect();

        let (parsed_topics, parsed_data) = BCFS::parse_log(&log).unwrap();
        assert_eq!(parsed_topics, topics);
        assert_eq!(parsed_data, data);
    }

    quickcheck::quickcheck! {
        fn parse_log_nopanic(inp: Vec<u8>) -> () {
            BCFS::parse_log(&inp);
        }
    }
}
