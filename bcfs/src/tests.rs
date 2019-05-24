#![cfg(test)]

use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use blockchain_traits::Blockchain;
use memchain::{Account, Memchain, BASE_GAS};
use oasis_types::Address;
use proptest::prelude::*;
use wasi_types::{
    ErrNo, Fd, FdFlags, FdStat, FileDelta, FileSize, FileStat, FileType, Inode, OpenFlags, Rights,
    Whence,
};

use crate::BCFS;

fn giga(val: u64) -> u64 {
    val * 1_000_000_000
}

fn create_memchain<'bc>(
    mains: Vec<Option<extern "C" fn(*const *mut dyn Blockchain<Address = Address>) -> u16>>,
) -> impl Blockchain<Address = Address> {
    let genesis_state = mains
        .into_iter()
        .enumerate()
        .map(|(i, main)| {
            let i = i + 1;
            (
                Address::from(i),
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
    p.push(hex::encode(&Address::from(1)));
    p
}

#[test]
fn close_fd() {
    let mut bc = create_memchain(vec![None]);
    let mut bcfs = BCFS::new(&mut bc, Address::from(1));
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
        let mut bcfs = BCFS::new(bc, Address::from(1));
        let mut abspath = good_home();
        abspath.push("somefile");
        let relpath = PathBuf::from("somefile");

        let abs_fd = bcfs
            .open(bc, None, &abspath, OpenFlags::CREATE, FdFlags::empty())
            .expect("Could not open");

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
    bc.transact(
        Address::from(2),
        Address::from(1),
        0,
        Vec::new(),
        BASE_GAS, /* gas */
        0,
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
        let mut bcfs = BCFS::new(&mut bc, Address::from(1));
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
        let mut bcfs = BCFS::new(&mut bc, Address::from(1));
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
        let mut bcfs = BCFS::new(&mut bc, Address::from(1));
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
        let mut bcfs = BCFS::new(&mut bc, Address::from(1));
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
        let mut bcfs = BCFS::new(&mut bc, Address::from(1));
        prop_assert!(bcfs.open(&mut bc, None, &path, of, ff,).is_err());
    }
}
