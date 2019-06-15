use std::{
    cell::Cell,
    convert::TryFrom,
    io::{IoSlice, IoSliceMut, Read, Write},
    path::{Path, PathBuf},
};

use blockchain_traits::{AccountMeta, Address, PendingTransaction, TransactionOutcome};
use wasi_types::{
    ErrNo, Fd, FdFlags, FdStat, FileDelta, FileSize, FileStat, FileType, Inode, OpenFlags, Rights,
    Whence,
};

use crate::{
    file::{File, FileKind, FileOffset, Filelike},
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

        let rel_path = path.strip_prefix(&self.home_dir).unwrap_or(path);

        if let Some(svc_file_kind) = self.is_service_path(ptx, rel_path) {
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
                offset: FileOffset::FromStart(0),
                flags: fd_flags,
                metadata: Cell::new(None),
            })));
            return Ok(fd);
        }

        if rel_path.is_absolute() {
            // there are no other special files and those outside of the home directory are
            // (currently) defined as not existing. This will change once services can pass
            // capabilities to their storage to other services.
            return Err(ErrNo::NoEnt);
        }

        let key = Self::key_for_path(rel_path)?;
        let file_exists = ptx.state().contains(&key);

        if file_exists && open_flags.contains(OpenFlags::EXCL) {
            return Err(ErrNo::Exist);
        } else if !file_exists && !open_flags.contains(OpenFlags::CREATE) {
            return Err(ErrNo::NoEnt);
        }

        let offset = if open_flags.contains(OpenFlags::TRUNC) {
            FileOffset::FromStart(0)
        } else if fd_flags.contains(FdFlags::APPEND) {
            FileOffset::FromEnd(0)
        } else {
            FileOffset::FromStart(0)
        };

        let fd = self.alloc_fd()?;
        self.files.push(Some(Filelike::File(File {
            kind: FileKind::Regular { key: key.to_vec() },
            offset,
            flags: fd_flags,
            metadata: Cell::new(if file_exists {
                None
            } else {
                ptx.state_mut().set(key, &Vec::new());
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
        })));
        Ok(fd)
    }

    pub fn open_service_sock(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        addr: A,
        value: u64,
        input: &[u8],
    ) -> Result<Fd> {
        let receipt = ptx.transact(addr, value, &input);
        match receipt.outcome() {
            TransactionOutcome::Success => (),
            err => return Err(tx_err_to_errno(err)),
        }
        let fd = self.alloc_fd()?;
        self.files.push(Some(Filelike::File(File {
            kind: FileKind::ServiceSock {
                addr: AnyAddress::Native(addr),
                receipt,
            },
            offset: FileOffset::Stream,
            flags: FdFlags::SYNC,
            metadata: Cell::new(Some(FileStat {
                device: 0u64.into(),
                inode: 0u32.into(),
                file_type: FileType::SocketStream,
                num_links: 0,
                file_size: 0,
                atime: 0u64.into(),
                mtime: 0u64.into(),
                ctime: 0u64.into(),
            })),
        })));
        Ok(fd)
    }

    pub fn close(
        &mut self,
        _ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<()> {
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
        let cur_offset = self.file_mut(fd)?.offset;
        if let FileOffset::Stream = cur_offset {
            return Err(ErrNo::SPipe);
        }
        match whence {
            Whence::Current => match cur_offset {
                FileOffset::FromStart(start_offset) => {
                    let new_offset =
                        Self::checked_offset(start_offset, offset).ok_or(ErrNo::Inval)?;
                    let mut file = self.file_mut(fd)?;
                    file.offset = FileOffset::FromStart(new_offset as u64);
                    Ok(new_offset as u64)
                }
                FileOffset::FromEnd(end_offset) => {
                    let filesize = self.filestat(ptx, fd)?.file_size;
                    let abs_offset = Self::checked_offset(
                        filesize,
                        end_offset.checked_add(offset).ok_or(ErrNo::Inval)?,
                    )
                    .ok_or(ErrNo::Inval)? as u64;
                    let mut file = self.file_mut(fd)?;
                    file.offset = FileOffset::FromStart(abs_offset);
                    Ok(abs_offset)
                }
                _ => unreachable!("Checked above"),
            },
            Whence::Start => {
                if offset < 0 {
                    return Err(ErrNo::Inval);
                }
                let mut file = self.file_mut(fd)?;
                file.offset = FileOffset::FromStart(offset as u64);
                Ok(offset as u64)
            }
            Whence::End => {
                let filesize = self.filestat(ptx, fd)?.file_size;
                let new_offset = Self::checked_offset(filesize, offset).ok_or(ErrNo::Inval)?;
                let mut file = self.file_mut(fd)?;
                file.offset = FileOffset::FromStart(new_offset as u64);
                Ok(new_offset as u64)
            }
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
        if let Some(meta) = file.metadata.get() {
            return Ok(meta);
        }
        let (file_size, file_type, inode) = match &file.kind {
            FileKind::Stdin => (
                ptx.input().len() as u64,
                FileType::RegularFile,
                u32::from(fd).into(),
            ),
            FileKind::Stdout | FileKind::Stderr | FileKind::Log => return Err(ErrNo::Inval),
            FileKind::Bytecode {
                addr: AnyAddress::Native(addr),
            } => (
                ptx.code_at(addr)
                    .map(|v| v.len() as u64)
                    .unwrap_or_default() as u64,
                FileType::RegularFile,
                Self::hash_inode(addr.as_ref(), "bytecode"),
            ),
            FileKind::Balance {
                addr: AnyAddress::Native(addr),
            } => (
                std::mem::size_of::<u64>() as u64,
                FileType::RegularFile,
                Self::hash_inode(addr.as_ref(), "balance"),
            ),
            FileKind::ServiceSock {
                addr: AnyAddress::Native(addr),
                ..
            } => (
                0,
                FileType::SocketStream,
                Self::hash_inode(&addr.as_ref(), "sock"),
            ),
            FileKind::Regular { key } => (
                ptx.state()
                    .get(key)
                    .map(|v| v.len() as u64)
                    .unwrap_or_default(),
                FileType::RegularFile,
                Self::hash_inode(key, ""),
            ),
            _ => return Err(ErrNo::Fault),
        };
        let meta = FileStat {
            device: 0u64.into(),
            inode, // TODO(#80)
            file_type,
            num_links: 0,
            file_size,
            atime: 0u64.into(), // TODO(#81)
            mtime: 0u64.into(),
            ctime: 0u64.into(),
        };
        file.metadata.set(Some(meta));
        Ok(meta)
    }

    pub fn tell(
        &self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
    ) -> Result<FileSize> {
        let file = self.file(fd)?;
        match file.offset {
            FileOffset::FromStart(o) => Ok(o),
            FileOffset::FromEnd(o) => {
                let filesize = self.filestat(ptx, fd)?.file_size;
                Ok(Self::checked_offset(filesize, o).ok_or(ErrNo::Inval)?)
            }
            FileOffset::Stream => Err(ErrNo::SPipe),
        }
    }

    pub fn read_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &mut [IoSliceMut],
    ) -> Result<usize> {
        let (nbytes, offset) = self.do_pread_vectored(ptx, fd, bufs, None)?;
        let mut file = self.file_mut(fd)?;
        file.offset = offset;
        Ok(nbytes)
    }

    pub fn pread_vectored(
        &self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &mut [IoSliceMut],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pread_vectored(ptx, fd, bufs, Some(FileOffset::FromStart(offset)))
            .map(|(nbytes, _offset)| nbytes)
    }

    pub fn write_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &[IoSlice],
    ) -> Result<usize> {
        let (nbytes, offset) = self.do_pwrite_vectored(ptx, fd, bufs, None)?;
        let mut file = self.file_mut(fd)?;
        file.offset = offset;
        Ok(nbytes)
    }

    pub fn pwrite_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &[IoSlice],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pwrite_vectored(ptx, fd, bufs, Some(FileOffset::FromStart(offset)))
            .map(|(nbytes, _offset)| nbytes)
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

fn tx_err_to_errno(tx_err: TransactionOutcome) -> ErrNo {
    match tx_err {
        TransactionOutcome::Success => ErrNo::Success,
        TransactionOutcome::InsufficientFunds => ErrNo::BadMsg,
        TransactionOutcome::InsufficientGas => ErrNo::DQuot,
        TransactionOutcome::InvalidInput => ErrNo::Inval,
        TransactionOutcome::NoAccount => ErrNo::AddrNotAvail,
        TransactionOutcome::Aborted => ErrNo::Canceled,
        _ => ErrNo::Io,
    }
}

impl<A: Address, M: AccountMeta> BCFS<A, M> {
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

    fn checked_offset(base: u64, offset: i64) -> Option<u64> {
        if offset >= 0 {
            base.checked_add(offset as u64)
        } else {
            base.checked_sub(-offset as u64)
        }
    }

    fn key_for_path(path: &Path) -> Result<&[u8]> {
        path.to_str().ok_or(ErrNo::Inval).map(str::as_bytes)
    }

    fn hash_inode(bytes: &[u8], disambiguator: &str) -> Inode {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(bytes);
        hasher.write(disambiguator.as_bytes());
        hasher.finish().into()
    }

    /// Returns (bytes_read, new_offset).
    fn do_pread_vectored(
        &self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &mut [IoSliceMut],
        offset: Option<FileOffset>,
    ) -> Result<(usize, FileOffset)> {
        let file = self.file(fd)?;

        if let FileOffset::Stream = file.offset {
            match offset {
                Some(FileOffset::FromStart(0)) | None => (),
                _ => return Err(ErrNo::SPipe),
            }
        }

        let read_offset = offset.unwrap_or(file.offset);
        let filesize = self.filestat(ptx, fd)?.file_size;
        match read_offset {
            FileOffset::FromStart(o) if o == 0 => (),
            FileOffset::FromEnd(o) if o == -(filesize as i64) => (),
            FileOffset::FromStart(o) if o == filesize => return Ok((0, file.offset)),
            FileOffset::FromEnd(o) if o == 0 => return Ok((0, file.offset)),
            _ => return Err(ErrNo::NotSup),
        }

        let nbytes = match &file.kind {
            FileKind::Stdout | FileKind::Stderr | FileKind::Log => return Err(ErrNo::Inval),
            FileKind::Stdin => ptx.input().as_slice().read_vectored(bufs)?,
            FileKind::Bytecode {
                addr: crate::AnyAddress::Native(addr),
            } => match ptx.code_at(addr) {
                Some(code) => code.to_vec().as_slice().read_vectored(bufs)?,
                None => return Err(ErrNo::NoEnt),
            },
            FileKind::Balance {
                addr: crate::AnyAddress::Native(addr),
            } => match ptx.account_meta_at(addr) {
                Some(meta) => meta.balance().to_le_bytes().as_ref().read_vectored(bufs)?,
                None => return Err(ErrNo::NoEnt),
            },
            FileKind::Regular { key } => {
                let mut bytes = match ptx.state().get(key) {
                    Some(bytes) => bytes,
                    None => return Err(ErrNo::NoEnt),
                };
                bytes.read_vectored(bufs)?
            }
            FileKind::ServiceSock { receipt, .. } => {
                receipt.output().as_slice().read_vectored(bufs)?
            }
            _ => return Err(ErrNo::Fault),
        };

        let new_offset = match file.offset {
            FileOffset::Stream => FileOffset::Stream,
            _ => FileOffset::FromStart(nbytes as u64),
        };

        Ok((nbytes, new_offset))
    }

    /// Returns (bytes_written, new_offset).
    fn do_pwrite_vectored(
        &mut self,
        ptx: &mut dyn PendingTransaction<Address = A, AccountMeta = M>,
        fd: Fd,
        bufs: &[IoSlice],
        offset: Option<FileOffset>,
    ) -> Result<(usize, FileOffset)> {
        let file = self.file(fd)?;
        let write_offset = offset.unwrap_or(file.offset);
        match file.kind {
            FileKind::Stdin
            | FileKind::Bytecode { .. }
            | FileKind::Balance { .. }
            | FileKind::ServiceSock { .. } => return Err(ErrNo::Inval),
            _ => (),
        };

        let mut cat_buf = Vec::with_capacity(bufs.iter().map(|v| v.len()).sum());
        let nbytes = cat_buf.write_vectored(bufs)?;

        let new_offset = match &file.kind {
            FileKind::Stdout => {
                ptx.ret(&cat_buf);
                FileOffset::FromEnd(0)
            }
            FileKind::Stderr => {
                ptx.err(&cat_buf);
                FileOffset::FromEnd(0)
            }
            FileKind::Log => {
                // NOTE: topics must be written as a block of TOPIC_SIZE * MAX_TOPICS bytes.
                // Space for unused topics should be set to zero.
                const TOPIC_SIZE: usize = 32; // 256 bits
                const MAX_TOPICS: usize = 4;
                let data = cat_buf.split_off(TOPIC_SIZE * MAX_TOPICS);
                let topics = cat_buf
                    .chunks_exact(TOPIC_SIZE)
                    .map(|c| {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(c);
                        arr
                    })
                    .collect::<Vec<[u8; 32]>>();
                ptx.emit(
                    &topics.iter().map(<[u8; 32]>::as_ref).collect::<Vec<_>>(),
                    &data,
                );
                FileOffset::FromEnd(0)
            }
            FileKind::Regular { key } => {
                let filesize = self.filestat(ptx, fd)?.file_size;
                match write_offset {
                    FileOffset::FromStart(0) => (),
                    FileOffset::FromEnd(o) if o == -(filesize as i64) => (),
                    _ => return Err(ErrNo::NotSup),
                }
                let nbytes = cat_buf.len();
                ptx.state_mut().set(key, &cat_buf);
                file.metadata.update(|mm| {
                    mm.map(|mut m| {
                        m.file_size = nbytes as u64;
                        m
                    })
                });
                match write_offset {
                    FileOffset::FromStart(o) => FileOffset::FromStart(o + nbytes as u64),
                    FileOffset::FromEnd(o) => {
                        FileOffset::FromEnd(filesize as i64 + o + nbytes as i64)
                    }
                    FileOffset::Stream => FileOffset::Stream,
                }
            }
            _ => unreachable!("checked above"),
        };

        Ok((nbytes, new_offset))
    }
}
