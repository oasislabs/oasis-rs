pub struct BCFS {
    backing: Rc<RefCell<dyn KVStore>>,
    bci: Rc<RefCell<dyn BlockchainIntrinsics>>,
    files: Vec<Option<Filelike>>,
    home_dir: PathBuf,
}

impl BCFS {
    /// Creates a new Blockchain FS with a backing KVStore and hex stringified owner address.
    pub fn new(
        backing: Rc<RefCell<dyn KVStore>>,
        bci: Rc<RefCell<dyn BlockchainIntrinsics>>,
        owner_addr: Address,
    ) -> Self {
        Self {
            backing,
            bci,
            files: vec![
                Some(Filelike::File(File::stdin())),
                Some(Filelike::File(File::stdout())),
                Some(Filelike::File(File::stderr())),
                Some(Filelike::File(File::log())),
            ],
            home_dir: Path::new("/").join(hex::encode(&owner_addr)),
        }
    }

    pub fn open(
        &mut self,
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
            return Ok(File::LOG_DESCRIPTOR.into());
        }

        let rel_path = path.strip_prefix(&self.home_dir).unwrap_or(path);

        if let Some(addr) = self.is_code_path(rel_path) {
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
                kind: FileKind::Bytecode { addr },
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

        let file_exists = self
            .backing
            .borrow()
            .contains(Self::key_for_path(rel_path)?);

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
            kind: FileKind::Regular {
                key: Self::key_for_path(rel_path)?.to_vec(),
            },
            offset,
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
        })));
        Ok(fd)
    }

    pub fn close(&mut self, fd: Fd) -> Result<()> {
        if self.file(fd)?.is_special() {
            return Ok(());
        }
        self.files[u32::from(fd) as usize]
            .take()
            .expect("Checked to exist above.");
        Ok(())
    }

    pub fn seek(&mut self, fd: Fd, offset: FileDelta, whence: Whence) -> Result<FileSize> {
        let cur_offset = self.file_mut(fd)?.offset;
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
                    let filesize = self.filestat(fd)?.file_size;
                    let abs_offset = Self::checked_offset(
                        filesize,
                        end_offset.checked_add(offset).ok_or(ErrNo::Inval)?,
                    )
                    .ok_or(ErrNo::Inval)? as u64;
                    let mut file = self.file_mut(fd)?;
                    file.offset = FileOffset::FromStart(abs_offset);
                    Ok(abs_offset)
                }
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
                let filesize = self.filestat(fd)?.file_size;
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

    pub fn fdstat(&self, fd: Fd) -> Result<FdStat> {
        let file = self.file(fd)?;
        Ok(FdStat {
            file_type: FileType::RegularFile,
            flags: file.flags,
            rights_base: Rights::all(),
            rights_inheriting: Rights::all(),
        })
    }

    pub fn filestat(&self, fd: Fd) -> Result<FileStat> {
        let file = self.file(fd)?;
        if let Some(meta) = file.metadata.get() {
            return Ok(meta);
        }
        let (file_size, inode) = match &file.kind {
            FileKind::Stdin => (self.bci.borrow().input_len(), u32::from(fd).into()),
            FileKind::Stdout | FileKind::Stderr | FileKind::Log => return Err(ErrNo::Inval),
            FileKind::Bytecode { addr } => (
                self.bci.borrow().code_len(addr),
                Self::hash_inode(addr.as_ref()),
            ),
            FileKind::Regular { key } => (self.backing.borrow().size(key), Self::hash_inode(key)),
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

    pub fn read_vectored(&mut self, fd: Fd, bufs: &mut [IoSliceMut]) -> Result<usize> {
        let (nbytes, offset) = self.do_pread_vectored(fd, bufs, None)?;
        let mut file = self.file_mut(fd)?;
        file.offset = offset;
        Ok(nbytes)
    }

    pub fn pread_vectored(
        &self,
        fd: Fd,
        bufs: &mut [IoSliceMut],
        offset: FileSize,
    ) -> Result<usize> {
        self.do_pread_vectored(fd, bufs, Some(FileOffset::FromStart(offset)))
            .map(|(nbytes, _offset)| nbytes)
    }

    pub fn write_vectored(&mut self, fd: Fd, bufs: &[IoSlice]) -> Result<usize> {
        let (nbytes, offset) = self.do_pwrite_vectored(fd, bufs, None)?;
        let mut file = self.file_mut(fd)?;
        file.offset = offset;
        Ok(nbytes)
    }

    pub fn pwrite_vectored(&mut self, fd: Fd, bufs: &[IoSlice], offset: FileSize) -> Result<usize> {
        self.do_pwrite_vectored(fd, bufs, Some(FileOffset::FromStart(offset)))
            .map(|(nbytes, _offset)| nbytes)
    }
}

impl BCFS {
    fn file(&self, fd: Fd) -> Result<&File> {
        match self
            .files
            .get(usize::try_from(u64::from(fd)).map_err(|_| ErrNo::BadF)?)
        {
            Some(Some(Filelike::File(file))) => Ok(file),
            _ => Err(ErrNo::BadF),
        }
    }

    fn file_mut(&mut self, fd: Fd) -> Result<&mut File> {
        match self
            .files
            .get_mut(usize::try_from(u64::from(fd)).map_err(|_| ErrNo::BadF)?)
        {
            Some(Some(Filelike::File(file))) => Ok(file),
            _ => Err(ErrNo::BadF),
        }
    }

    /// Returns S
    fn is_code_path(&self, path: &Path) -> Option<Address> {
        use std::path::Component;

        if path == Path::new("code") {
            return Some(Address::from_slice(
                &hex::decode(
                    self.home_dir
                        .file_name()
                        .expect("`home_dir` is constructed from `owner_addr`")
                        .to_str()
                        .expect("Runtime should have passed in a hex string for `owner_addr`"),
                )
                .expect("`home_dir` was constructed from `hex::encode`"),
            ));
        }

        let mut comps = path.components();

        match comps.next() {
            Some(Component::RootDir) => (),
            _ => return None,
        }

        let addr = match hex::decode(match comps.next().map(|c| c.as_os_str().to_str()) {
            Some(Some(maybe_addr)) => maybe_addr,
            _ => return None,
        }) {
            Ok(addr) if addr.len() == Address::len_bytes() => Address::from_slice(&addr),
            _ => return None,
        };

        match comps.next() {
            None => Some(addr),
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

    fn hash_inode(bytes: &[u8]) -> Inode {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(bytes);
        hasher.finish().into()
    }

    fn do_pread_vectored(
        &self,
        fd: Fd,
        bufs: &mut [IoSliceMut],
        offset: Option<FileOffset>,
    ) -> Result<(usize, FileOffset)> {
        let file = self.file(fd)?;
        let read_offset = offset.unwrap_or_else(|| file.offset);
        match read_offset {
            FileOffset::FromEnd(0) => return Ok((0, file.offset)),
            FileOffset::FromStart(o) if o != 0 => return Err(ErrNo::NotSup),
            FileOffset::FromEnd(o) if o != 0 => return Err(ErrNo::NotSup),
            _ => (),
        }

        match &file.kind {
            FileKind::Stdout | FileKind::Stderr | FileKind::Log => Err(ErrNo::Inval),
            FileKind::Stdin => {
                let nbytes = self.bci.borrow().input().as_slice().read_vectored(bufs)?;
                Ok((nbytes, FileOffset::FromStart(nbytes as u64)))
            }
            FileKind::Bytecode { addr } => match self.bci.borrow().code_at(addr) {
                Some(code) => {
                    let nbytes = code.as_slice().read_vectored(bufs)?;
                    Ok((nbytes, FileOffset::FromStart(nbytes as u64)))
                }
                None => Err(ErrNo::NoEnt),
            },
            FileKind::Regular { key } => {
                let kvs = self.backing.borrow();
                let mut bytes = match kvs.get(key) {
                    Some(bytes) => bytes,
                    None => return Err(ErrNo::NoEnt),
                };
                Ok((bytes.read_vectored(bufs)?, FileOffset::FromEnd(0)))
            }
        }
    }

    fn do_pwrite_vectored(
        &self,
        fd: Fd,
        bufs: &[IoSlice],
        offset: Option<FileOffset>,
    ) -> Result<(usize, FileOffset)> {
        let file = self.file(fd)?;
        let write_offset = offset.unwrap_or_else(|| file.offset);
        match file.kind {
            FileKind::Stdin | FileKind::Bytecode { .. } => return Err(ErrNo::Inval),
            _ => (),
        };

        let mut cat_buf = Vec::with_capacity(bufs.iter().map(|v| v.len()).sum());
        let nbytes = cat_buf.write_vectored(bufs)?;

        let new_offset = match &file.kind {
            FileKind::Stdout => {
                self.bci.borrow_mut().ret(cat_buf);
                FileOffset::FromEnd(0)
            }
            FileKind::Stderr => {
                self.bci.borrow_mut().ret_err(cat_buf);
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
                self.bci.borrow_mut().emit(topics, data);
                FileOffset::FromEnd(0)
            }
            FileKind::Regular { key } => {
                match write_offset {
                    FileOffset::FromStart(0) => (),
                    _ => return Err(ErrNo::NotSup),
                }
                self.backing.borrow_mut().set(key.to_vec(), cat_buf);
                match write_offset {
                    FileOffset::FromStart(o) => FileOffset::FromStart(o + nbytes as u64),
                    FileOffset::FromEnd(_) => FileOffset::FromEnd(0),
                }
            }
            FileKind::Stdin | FileKind::Bytecode { .. } => unreachable!("checked above"),
        };
        Ok((nbytes, new_offset))
    }
}
