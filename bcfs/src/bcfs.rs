use std::{
    cell::Cell,
    convert::TryFrom,
    io::{IoSlice, IoSliceMut, Read, Write},
    path::{Path, PathBuf},
};

use blockchain_traits::{Address, Blockchain};
use wasi_types::{
    ErrNo, Fd, FdFlags, FdStat, FileDelta, FileSize, FileStat, FileType, Inode, OpenFlags, Rights,
    Whence,
};

use crate::{
    file::{File, FileKind, FileOffset, Filelike},
    MultiAddress, Result,
};

pub struct BCFS<A: Address> {
    files: Vec<Option<Filelike<A>>>,
    context_addr: A,
    home_dir: PathBuf,
}

impl<A: Address> BCFS<A> {
    /// Creates a new Blockchain FS with a backing `Blockchain` and hex stringified
    /// owner address.
    pub fn new(blockchain: &mut dyn Blockchain<Address = A>, context_addr: A) -> Self {
        let mut home_dir = PathBuf::from("/opt");
        home_dir.push(blockchain.name());
        home_dir.push(context_addr.path_repr());

        Self {
            files: vec![
                Some(Filelike::File(File::stdin())),
                Some(Filelike::File(File::stdout())),
                Some(Filelike::File(File::stderr())),
                Some(Filelike::File(File::log())),
            ],
            home_dir,
            context_addr,
        }
    }

    pub fn open(
        &mut self,
        blockchain: &mut dyn Blockchain<Address = A>,
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

        if let Some(svc_file_kind) = self.is_service_path(blockchain, rel_path) {
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
        let file_exists = blockchain
            .contains(&self.context_addr, &key)
            .map_err(kverror_to_errno)?;

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
                blockchain
                    .set(&self.context_addr, key.to_vec(), Vec::new())
                    .map_err(kverror_to_errno)?;
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

    pub fn close(&mut self, _blockchain: &mut dyn Blockchain<Address = A>, fd: Fd) -> Result<()> {
        match self.files.get_mut(u32::from(fd) as usize) {
            Some(f) if f.is_some() => {
                *f = None;
                return Ok(());
            }
            _ => return Err(ErrNo::BadF),
        }
    }

    pub fn seek(
        &mut self,
        blockchain: &mut dyn Blockchain<Address = A>,
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
                    let filesize = self.filestat(blockchain, fd)?.file_size;
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
                let filesize = self.filestat(blockchain, fd)?.file_size;
                let new_offset = Self::checked_offset(filesize, offset).ok_or(ErrNo::Inval)?;
                if offset < 0 {
                    return Err(ErrNo::Inval);
                }
                let mut file = self.file_mut(fd)?;
                file.offset = FileOffset::FromStart(new_offset as u64);
                Ok(new_offset as u64)
            }
        }
    }

    pub fn fdstat(&self, _blockchain: &mut dyn Blockchain<Address = A>, fd: Fd) -> Result<FdStat> {
        let file = self.file(fd)?;
        Ok(FdStat {
            file_type: FileType::RegularFile,
            flags: file.flags,
            rights_base: Rights::all(),
            rights_inheriting: Rights::all(),
        })
    }

    pub fn filestat(&self, blockchain: &dyn Blockchain<Address = A>, fd: Fd) -> Result<FileStat> {
        let file = self.file(fd)?;
        if let Some(meta) = file.metadata.get() {
            return Ok(meta);
        }
        let (file_size, inode) = match &file.kind {
            FileKind::Stdin => (blockchain.input_len(), u32::from(fd).into()),
            FileKind::Stdout | FileKind::Stderr | FileKind::Log => return Err(ErrNo::Inval),
            FileKind::Bytecode {
                addr: MultiAddress::Native(addr),
            } => (
                blockchain.code_len(addr),
                Self::hash_inode(addr.as_ref(), "bytecode"),
            ),
            FileKind::Balance {
                addr: MultiAddress::Native(addr),
            } => (
                std::mem::size_of::<u64>() as u64,
                Self::hash_inode(addr.as_ref(), "balance"),
            ),
            FileKind::ServiceSock {
                addr: MultiAddress::Native(addr),
            } => (0, Self::hash_inode(addr.as_ref(), "sock")),
            FileKind::Regular { key } => (
                blockchain
                    .size(&self.context_addr, key)
                    .map_err(kverror_to_errno)?,
                Self::hash_inode(key, ""),
            ),
            _ => return Err(ErrNo::Fault),
        };
        let meta = FileStat {
            device: 0u64.into(),
            inode, // TODO: directories and inodes
            file_type: FileType::RegularFile,
            num_links: 0,
            file_size,
            atime: 0u64.into(), // TODO: timestamps
            mtime: 0u64.into(),
            ctime: 0u64.into(),
        };
        file.metadata.set(Some(meta));
        Ok(meta)
    }

    pub fn read_vectored(
        &mut self,
        blockchain: &mut dyn Blockchain<Address = A>,
        fd: Fd,
        bufs: &mut [IoSliceMut],
    ) -> Result<usize> {
        let (nbytes, offset) = self.do_pread_vectored(blockchain, fd, bufs, None)?;
        let mut file = self.file_mut(fd)?;
        file.offset = offset;
        Ok(nbytes)
    }

    pub fn pread_vectored(
        &self,
        blockchain: &mut dyn Blockchain<Address = A>,
        fd: Fd,
        bufs: &mut [IoSliceMut],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pread_vectored(blockchain, fd, bufs, Some(FileOffset::FromStart(offset)))
            .map(|(nbytes, _offset)| nbytes)
    }

    pub fn write_vectored(
        &mut self,
        blockchain: &mut dyn Blockchain<Address = A>,
        fd: Fd,
        bufs: &[IoSlice],
    ) -> Result<usize> {
        let (nbytes, offset) = self.do_pwrite_vectored(blockchain, fd, bufs, None)?;
        let mut file = self.file_mut(fd)?;
        file.offset = offset;
        Ok(nbytes)
    }

    pub fn pwrite_vectored(
        &mut self,
        blockchain: &mut dyn Blockchain<Address = A>,
        fd: Fd,
        bufs: &[IoSlice],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pwrite_vectored(blockchain, fd, bufs, Some(FileOffset::FromStart(offset)))
            .map(|(nbytes, _offset)| nbytes)
    }
}

impl<A: Address> BCFS<A> {
    fn file(&self, fd: Fd) -> Result<&File<A>> {
        match self
            .files
            .get(usize::try_from(u64::from(fd)).map_err(|_| ErrNo::BadF)?)
        {
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
        blockchain: &dyn Blockchain<Address = A>,
        path: &Path,
    ) -> Option<FileKind<A>> {
        use std::path::Component;

        if path == Path::new("code") {
            return Some(FileKind::Bytecode {
                addr: MultiAddress::Native(self.context_addr),
            });
        } else if path == Path::new("balance") {
            return Some(FileKind::Balance {
                addr: MultiAddress::Native(self.context_addr),
            });
        } else if path == Path::new("sock") {
            // why would a service call itself?
            return Some(FileKind::ServiceSock {
                addr: MultiAddress::Native(self.context_addr),
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
            Some(Component::Normal(c)) if c == blockchain.name() => (),
            _ => return None,
        }

        let addr = match comps
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .map(A::from_str)
        {
            Some(Ok(addr)) => MultiAddress::Native(addr),
            _ => return None,
        };

        let svc_file_kind = match comps.next() {
            Some(Component::Normal(c)) if c == "code" => FileKind::Bytecode { addr },
            Some(Component::Normal(c)) if c == "balance" => FileKind::Balance { addr },
            Some(Component::Normal(c)) if c == "sock" => FileKind::ServiceSock { addr },
            _ => return None,
        };

        match comps.next() {
            None => Some(svc_file_kind),
            _ => None,
        }
    }

    fn alloc_fd(&self) -> Result<Fd> {
        if self.files.len() >= u32::max_value() as usize {
            return Err(ErrNo::NFile); // TODO: handle closed FDs
        }
        Ok((self.files.len() as u32).into())
    }

    fn checked_offset(base: u64, offset: i64) -> Option<u64> {
        if offset >= 0 {
            base.checked_add(offset as u64)
        } else {
            base.checked_sub(offset as u64)
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
        blockchain: &mut dyn Blockchain<Address = A>,
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

        let read_offset = offset.unwrap_or_else(|| file.offset);
        match read_offset {
            FileOffset::FromEnd(0) => return Ok((0, file.offset)),
            FileOffset::FromStart(o) if o != 0 => return Err(ErrNo::NotSup),
            FileOffset::FromEnd(o) if o != 0 => return Err(ErrNo::NotSup),
            _ => (),
        }

        let (nbytes, mut new_offset) = match &file.kind {
            FileKind::Stdout | FileKind::Stderr | FileKind::Log => Err(ErrNo::Inval),
            FileKind::Stdin => {
                let nbytes = blockchain.fetch_input().as_slice().read_vectored(bufs)?;
                Ok((nbytes, FileOffset::FromStart(nbytes as u64)))
            }
            FileKind::Bytecode {
                addr: crate::MultiAddress::Native(addr),
            } => match blockchain.code_at(addr) {
                Some(code) => {
                    let nbytes = code.to_vec().as_slice().read_vectored(bufs)?;
                    Ok((nbytes, FileOffset::FromStart(nbytes as u64)))
                }
                None => Err(ErrNo::NoEnt),
            },
            FileKind::Balance {
                addr: crate::MultiAddress::Native(addr),
            } => match blockchain.metadata_at(addr) {
                Some(meta) => {
                    let nbytes = meta.balance.to_le_bytes().as_ref().read_vectored(bufs)?;
                    Ok((nbytes, FileOffset::FromStart(nbytes as u64)))
                }
                None => Err(ErrNo::NoEnt),
            },
            FileKind::Regular { key } => {
                let mut bytes = match blockchain
                    .get(&self.context_addr, key)
                    .map_err(kverror_to_errno)?
                {
                    Some(bytes) => bytes,
                    None => return Err(ErrNo::NoEnt),
                };
                Ok((bytes.read_vectored(bufs)?, FileOffset::FromEnd(0)))
            }
            _ => return Err(ErrNo::Fault),
        }?;

        if let FileOffset::Stream = file.offset {
            new_offset = file.offset;
        }

        Ok((nbytes, new_offset))
    }

    /// Returns (bytes_written, new_offset).
    fn do_pwrite_vectored(
        &mut self,
        blockchain: &mut dyn Blockchain<Address = A>,
        fd: Fd,
        bufs: &[IoSlice],
        offset: Option<FileOffset>,
    ) -> Result<(usize, FileOffset)> {
        let file = self.file(fd)?;
        let write_offset = offset.unwrap_or_else(|| file.offset);
        match file.kind {
            FileKind::ServiceSock {
                addr: MultiAddress::Foreign(_),
            } => return Err(ErrNo::NotSup),
            FileKind::Stdin | FileKind::Bytecode { .. } | FileKind::Balance { .. } => {
                return Err(ErrNo::Inval)
            }
            _ => (),
        };

        let mut cat_buf = Vec::with_capacity(bufs.iter().map(|v| v.len()).sum());
        let nbytes = cat_buf.write_vectored(bufs)?;

        let new_offset = match &file.kind {
            FileKind::Stdout => {
                blockchain.ret(cat_buf);
                FileOffset::FromEnd(0)
            }
            FileKind::Stderr => {
                blockchain.err(cat_buf);
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
                    .collect();
                blockchain.emit(topics, data);
                FileOffset::FromEnd(0)
            }
            FileKind::Regular { key } => {
                match write_offset {
                    FileOffset::FromStart(0) => (),
                    _ => return Err(ErrNo::NotSup),
                }
                blockchain
                    .set(&self.context_addr, key.to_vec(), cat_buf)
                    .map_err(kverror_to_errno)?;
                match write_offset {
                    FileOffset::FromStart(o) => FileOffset::FromStart(o + nbytes as u64),
                    FileOffset::FromEnd(_) => FileOffset::FromEnd(0),
                    FileOffset::Stream => FileOffset::Stream,
                }
            }
            FileKind::ServiceSock {
                addr: MultiAddress::Native(callee),
            } => {
                // TODO: how will wasi even expose value/gas/gas_price args to new process?
                blockchain.transact(
                    self.context_addr,
                    *callee,
                    0,       /* value */
                    cat_buf, /* input */
                    0,       /* gas */
                    0,       /* gas_price */
                );
                FileOffset::Stream
            }
            _ => unreachable!("checked above"),
        };
        Ok((nbytes, new_offset))
    }
}

fn kverror_to_errno(kverr: blockchain_traits::KVError) -> ErrNo {
    match kverr {
        blockchain_traits::KVError::NoAccount => ErrNo::Fault,
        blockchain_traits::KVError::NoPermission => ErrNo::Access,
        blockchain_traits::KVError::InvalidState => ErrNo::NotRecoverable,
    }
}
