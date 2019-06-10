#![cfg(test)]

use std::{
    borrow::Cow,
    collections::HashMap,
    io::{IoSlice, IoSliceMut},
    path::PathBuf,
};

use blockchain_traits::Blockchain;
use mantle_types::Address;
use memchain::{Account, Memchain, BASE_GAS};
use proptest::prelude::*;
use wasi_types::{ErrNo, Fd, FdFlags, OpenFlags, Whence};

use crate::BCFS;

const ADDR_1: Address = Address([1u8; 20]);
const ADDR_2: Address = Address([2u8; 20]);

fn giga(val: u64) -> u64 {
    val * 1_000_000_000
}

fn create_memchain(
    mains: Vec<Option<extern "C" fn(*const *mut dyn Blockchain<Address = Address>) -> u16>>,
) -> impl Blockchain<Address = Address> {
    let genesis_state = mains
        .into_iter()
        .enumerate()
        .map(|(i, main)| {
            let i = i + 1;
            (
                Address([i as u8; 20]),
                Cow::Owned(Account {
                    balance: giga(i as u64),
                    code: format!("\0asm not wasm {}", i).into_bytes(),
                    storage: {
                        let mut storage = HashMap::new();
                        storage.insert(
                            "common_key".to_string().into_bytes(),
                            "common_value".to_string().into_bytes(),
                        );
                        storage.insert(
                            format!("key_{}", i).into_bytes(),
                            format!("value_{}", i).into_bytes(),
                        );
                        storage
                    },
                    expiry: None,
                    main,
                }),
            )
        })
        .collect();

    Memchain::new("bcfs".to_string(), genesis_state)
}

/// Returns a known-good home directory.
fn good_home() -> PathBuf {
    let mut p = PathBuf::from("/opt/bcfs");
    p.push(hex::encode(&ADDR_1));
    p
}

#[test]
fn close_fd() {
    let mut bc = create_memchain(vec![None]);
    let mut bcfs = BCFS::new(&mut bc, ADDR_1);
    for fd in 0u32..=3 {
        let fd = Fd::from(fd);
        assert!(bcfs.close(&mut bc, fd).is_ok());
        assert_eq!(bcfs.close(&mut bc, fd), Err(ErrNo::BadF)); // double close
    }
    for fd in 4u32..10 {
        assert_eq!(bcfs.close(&mut bc, Fd::from(fd)), Err(ErrNo::BadF));
    }
}

#[test]
fn open_close() {
    extern "C" fn open_close_main(bc: *const *mut dyn Blockchain<Address = Address>) -> u16 {
        let bc = unsafe { &mut **bc };
        let mut bcfs = BCFS::new(bc, ADDR_1);
        let mut abspath = good_home();
        abspath.push("somefile");
        let relpath = PathBuf::from("somefile");

        let abs_fd = bcfs
            .open(bc, None, &abspath, OpenFlags::CREATE, FdFlags::empty())
            .unwrap();

        // double create
        assert_eq!(
            bcfs.open(bc, None, &abspath, OpenFlags::EXCL, FdFlags::empty()),
            Err(ErrNo::Exist)
        );

        let abs_fd2 = bcfs
            .open(bc, None, &abspath, OpenFlags::empty(), FdFlags::empty())
            .unwrap();
        let rel_fd = bcfs
            .open(bc, None, &relpath, OpenFlags::empty(), FdFlags::APPEND)
            .unwrap();

        assert!(bcfs.close(bc, abs_fd).is_ok());
        assert!(bcfs.close(bc, abs_fd2).is_ok());
        assert!(bcfs.close(bc, rel_fd).is_ok());
        0
    }

    let mut bc = create_memchain(vec![Some(open_close_main), None]);
    bc.transact(ADDR_2, ADDR_1, 0, Vec::new(), BASE_GAS /* gas */, 0);
}

#[test]
fn read_write_basic() {
    let mut bc = create_memchain(vec![None]);
    let mut bcfs = BCFS::new(&mut bc, ADDR_1);

    let path = PathBuf::from("somefile");

    let fd = bcfs
        .open(&mut bc, None, &path, OpenFlags::CREATE, FdFlags::empty())
        .unwrap();

    let write_bufs = ["hello", "world"];
    let mut read_bufs = write_bufs
        .iter()
        .map(|b| vec![0u8; b.len()])
        .collect::<Vec<_>>();
    let nbytes = write_bufs.iter().map(|b| b.len()).sum();

    macro_rules! assert_read {
        ($read_bufs:ident, $write_bufs:ident, $nbytes:expr) => {
            assert_eq!(
                bcfs.read_vectored(
                    &mut bc,
                    fd,
                    &mut $read_bufs
                        .iter_mut()
                        .map(|b| IoSliceMut::new(b))
                        .collect::<Vec<_>>()
                ),
                Ok($nbytes)
            );
            assert!(
                $nbytes == 0
                    || std::str::from_utf8(&$read_bufs[0]).unwrap() == $write_bufs[0]
                        && std::str::from_utf8(&$read_bufs[1]).unwrap() == $write_bufs[1]
            );
        };
    }
    assert_read!(read_bufs, write_bufs, 0);

    assert_eq!(
        bcfs.write_vectored(
            &mut bc,
            fd,
            &write_bufs
                .iter()
                .map(|b| IoSlice::new(b.as_bytes()))
                .collect::<Vec<_>>()
        ),
        Ok(nbytes)
    );
    assert_read!(read_bufs, write_bufs, 0);

    assert_eq!(bcfs.seek(&mut bc, fd, 0, Whence::Start), Ok(0));
    assert_read!(read_bufs, write_bufs, nbytes);
    assert_read!(read_bufs, write_bufs, 0);

    assert_eq!(bcfs.seek(&mut bc, fd, -(nbytes as i64), Whence::End), Ok(0));
    assert_read!(read_bufs, write_bufs, nbytes);
    assert_read!(read_bufs, write_bufs, 0);

    assert_eq!(
        bcfs.seek(&mut bc, fd, -(nbytes as i64 - 2), Whence::Current),
        Ok(2)
    );
    assert_eq!(bcfs.seek(&mut bc, fd, -2, Whence::Current), Ok(0));

    assert_eq!(bcfs.seek(&mut bc, fd, 0, Whence::End), Ok(nbytes as u64));
    let write_bufs = ["hello", "blockchain"];
    let mut read_bufs = write_bufs
        .iter()
        .map(|b| vec![0u8; b.len()])
        .collect::<Vec<_>>();
    let new_nbytes = write_bufs.iter().map(|b| b.len()).sum();
    assert_eq!(
        bcfs.pwrite_vectored(
            &mut bc,
            fd,
            &write_bufs
                .iter()
                .map(|b| IoSlice::new(b.as_bytes()))
                .collect::<Vec<_>>(),
            0
        ),
        Ok(new_nbytes)
    );
    assert_eq!(bcfs.tell(&mut bc, fd), Ok(nbytes as u64));
    assert_eq!(
        bcfs.pread_vectored(
            &mut bc,
            fd,
            &mut read_bufs
                .iter_mut()
                .map(|b| IoSliceMut::new(b))
                .collect::<Vec<_>>(),
            0
        ),
        Ok(new_nbytes)
    );
    assert_eq!(std::str::from_utf8(&read_bufs[0]).unwrap(), write_bufs[0]);
    assert_eq!(std::str::from_utf8(&read_bufs[1]).unwrap(), write_bufs[1]);
}

#[test]
fn read_write_aliased() {
    let mut bc = create_memchain(vec![None]);
    let mut bcfs = BCFS::new(&mut bc, ADDR_1);

    let path = PathBuf::from("somefile");
    let abspath = good_home().join(&path);

    let abs_fd = bcfs
        .open(&mut bc, None, &path, OpenFlags::CREATE, FdFlags::empty())
        .unwrap();
    let rel_fd = bcfs
        .open(
            &mut bc,
            None,
            &abspath,
            OpenFlags::empty(),
            FdFlags::empty(),
        )
        .unwrap();

    let write_bufs = ["hello", "world"];
    let mut read_bufs = write_bufs
        .iter()
        .map(|b| vec![0u8; b.len()])
        .collect::<Vec<_>>();
    let nbytes = write_bufs.iter().map(|b| b.len()).sum();

    assert_eq!(
        bcfs.write_vectored(
            &mut bc,
            abs_fd, // NB: absolute path fd
            &write_bufs
                .iter()
                .map(|b| IoSlice::new(b.as_bytes()))
                .collect::<Vec<_>>()
        ),
        Ok(nbytes)
    );
    assert_eq!(
        bcfs.read_vectored(
            &mut bc,
            rel_fd, // NB: relative path fd
            &mut read_bufs
                .iter_mut()
                .map(|b| IoSliceMut::new(b))
                .collect::<Vec<_>>()
        ),
        Ok(nbytes)
    );
    assert_eq!(std::str::from_utf8(&read_bufs[0]).unwrap(), write_bufs[0]);
    assert_eq!(std::str::from_utf8(&read_bufs[1]).unwrap(), write_bufs[1]);
}

#[test]
fn badf() {
    let mut bc = create_memchain(vec![None]);
    let mut bcfs = BCFS::new(&mut bc, ADDR_1);

    let badf = Fd::from(99u32);

    assert_eq!(
        bcfs.read_vectored(&mut bc, badf, &mut Vec::new()),
        Err(ErrNo::BadF)
    );

    assert_eq!(
        bcfs.write_vectored(&mut bc, badf, &Vec::new()),
        Err(ErrNo::BadF)
    );

    assert_eq!(
        bcfs.pread_vectored(&mut bc, badf, &mut Vec::new(), 0),
        Err(ErrNo::BadF)
    );

    assert_eq!(
        bcfs.pwrite_vectored(&mut bc, badf, &Vec::new(), 0),
        Err(ErrNo::BadF)
    );

    assert_eq!(bcfs.seek(&mut bc, badf, 0, Whence::Start), Err(ErrNo::BadF));

    assert_eq!(bcfs.fdstat(&mut bc, badf).unwrap_err(), ErrNo::BadF);
    assert_eq!(bcfs.filestat(&bc, badf).unwrap_err(), ErrNo::BadF);
    assert_eq!(bcfs.tell(&mut bc, badf).unwrap_err(), ErrNo::BadF);
    assert_eq!(bcfs.renumber(&mut bc, badf, badf).unwrap_err(), ErrNo::BadF);

    assert_eq!(bcfs.close(&mut bc, badf), Err(ErrNo::BadF));
}

#[test]
fn renumber() {
    let mut bc = create_memchain(vec![None]);
    let mut bcfs = BCFS::new(&mut bc, ADDR_1);

    let somefile = PathBuf::from("somefile");
    let anotherfile = PathBuf::from("anotherfile");

    let somefile_fd = bcfs
        .open(
            &mut bc,
            None,
            &somefile,
            OpenFlags::CREATE,
            FdFlags::empty(),
        )
        .unwrap();
    let anotherfile_fd = bcfs
        .open(
            &mut bc,
            None,
            &anotherfile,
            OpenFlags::CREATE,
            FdFlags::empty(),
        )
        .unwrap();

    let write_bufs = ["destination", "somefile"];
    bcfs.write_vectored(
        &mut bc,
        somefile_fd,
        &write_bufs
            .iter()
            .map(|b| IoSlice::new(b.as_bytes()))
            .collect::<Vec<_>>(),
    )
    .unwrap();

    bcfs.renumber(&mut bc, somefile_fd, anotherfile_fd).unwrap();

    assert_eq!(
        bcfs.read_vectored(&mut bc, somefile_fd, &mut Vec::new()),
        Err(ErrNo::BadF)
    );

    let mut read_buf = vec![0u8; 1];
    assert_eq!(
        bcfs.pread_vectored(
            &mut bc,
            anotherfile_fd,
            &mut [IoSliceMut::new(&mut read_buf)],
            0,
        ),
        Ok(read_buf.len())
    );
    assert_eq!(
        bcfs.tell(&mut bc, anotherfile_fd),
        Ok(write_bufs.iter().map(|b| b.len() as u64).sum())
    );
}

proptest! {
    #[test]
    fn open_nonexistent_fail(
        root in "/(opt|\\PC*)",
        chain in "(bcfs|\\PC*)",
        addr in "[0-9A-Fa-f]{20,}",
        ext in "(code|balance|sock|\\PC*)"
    ) {
        let mut p = PathBuf::from(root);
        p.push(chain);
        p.push(addr);
        p.push(ext);

        let mut bc = create_memchain(vec![None]);
        let mut bcfs = BCFS::new(&mut bc, ADDR_1);
        prop_assert_eq!(
            bcfs.open(&mut bc, None, &p, OpenFlags::CREATE, FdFlags::empty()),
            Err(ErrNo::NoEnt)
        );
    }

    #[test]
    fn open_storage_nocreate_fail(
        of in (0u16..(1 << 2))
            .prop_map(|b| OpenFlags::from_bits(b << 2).unwrap()), // no create, no dir
        ff in (0u16..(1 << 5)).prop_map(|b| FdFlags::from_bits(b).unwrap()),
    ) {
        let mut p = good_home();
        p.push("somefile");

        let mut bc = create_memchain(vec![None]);
        let mut bcfs = BCFS::new(&mut bc, ADDR_1);
        prop_assert_eq!(bcfs.open(&mut bc, None, &p, of, ff,), Err(ErrNo::NoEnt));
    }

    #[test]
    fn open_storage_create_ok(
        mut path in prop::bool::ANY.prop_map(|abs| if abs { good_home() } else { PathBuf::new() }),
        ff in (0u16..(1 << 5)).prop_map(|b| FdFlags::from_bits(b).unwrap()),
        ext in "\\w+"
    ) {
        path.push(ext);
        let mut bc = create_memchain(vec![None]);
        let mut bcfs = BCFS::new(&mut bc, ADDR_1);
        let fd = bcfs.open(&mut bc, None, &path, OpenFlags::CREATE, ff).unwrap();
        prop_assert!(bcfs.close(&mut bc, fd).is_ok());
    }

    #[test]
    fn open_svc_ok(
        mut path in prop::bool::ANY.prop_map(|abs| if abs { good_home() } else { PathBuf::new() }),
        ext in "(code|balance|sock)",
        ff in (0u16..(1 << 4)).prop_map(|b| FdFlags::from_bits(b << 1).unwrap()), // no append
    ) {
        path.push(ext);
        let mut bc = create_memchain(vec![None]);
        let mut bcfs = BCFS::new(&mut bc, ADDR_1);
        let fd = bcfs.open(&mut bc, None, &path, OpenFlags::empty(), ff).unwrap();
        prop_assert!(bcfs.close(&mut bc, fd).is_ok());
    }

    #[test]
    fn open_svc_fail(
        mut path in prop::bool::ANY.prop_map(|abs| if abs { good_home() } else { PathBuf::new() }),
        ext in "(code|balance|sock)",
        of in (0u16..(1 << 4)).prop_map(|b| OpenFlags::from_bits(b | 1).unwrap()),
        ff in (0u16..(1 << 5)).prop_map(|b| FdFlags::from_bits(b | 1).unwrap()),
    ) {
        path.push(ext);
        let mut bc = create_memchain(vec![None]);
        let mut bcfs = BCFS::new(&mut bc, ADDR_1);
        prop_assert!(bcfs.open(&mut bc, None, &path, of, ff,).is_err());
    }
}
