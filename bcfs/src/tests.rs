#![cfg(test)]

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

use blockchain_traits::Blockchain;
use memchain::{Account, Memchain};
use oasis_types::Address;
use proptest::prelude::*;
use wasi_types::{
    ErrNo, FdFlags, FdStat, FileDelta, FileSize, FileStat, FileType, Inode, OpenFlags, Rights,
    Whence,
};

use crate::BCFS;

fn giga(val: u64) -> u64 {
    val * 1_000_000_000
}

fn create_memchain<'bc>(
    mains: Vec<Option<extern "C" fn(*mut dyn Blockchain<Address = Address>) -> u16>>,
) -> Rc<RefCell<Memchain<'bc>>> {
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

fn create_bcfs() -> BCFS<Address> {
    let bc = create_memchain(vec![None]);
    BCFS::new(bc, Address::from(1))
}

/// Returns a known-good path.
fn good_path() -> PathBuf {
    let mut p = PathBuf::from("/opt/bcfs");
    p.push(hex::encode(&Address::from(1)));
    p
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
        prop_assert_eq!(
            create_bcfs().open(None, &p, OpenFlags::CREATE, FdFlags::empty()),
            Err(ErrNo::NoEnt)
        );
    }

    #[test]
    fn open_storage_nocreate_fail(
        of in (0u16..(1 << 2))
            .prop_map(|b| OpenFlags::from_bits(b << 2).unwrap()), // no create, no dir
        ff in (0u16..(1 << 5)).prop_map(|b| FdFlags::from_bits(b).unwrap()),
    ) {
        let mut p = good_path();
        p.push("somefile");
        prop_assert_eq!(create_bcfs().open(None, &p, of, ff,), Err(ErrNo::NoEnt));
    }

    #[test]
    fn open_storage_create_success(
        mut path in prop::bool::ANY.prop_map(|abs| if abs { good_path() } else { PathBuf::new() }),
        ff in (0u16..(1 << 5)).prop_map(|b| FdFlags::from_bits(b).unwrap()),
        ext in "\\w+"
    ) {
        path.push(ext);
        prop_assert!(create_bcfs().open(None, &path, OpenFlags::CREATE, ff).is_ok());
    }

    #[test]
    fn open_svc_ok(
        mut path in prop::bool::ANY.prop_map(|abs| if abs { good_path() } else { PathBuf::new() }),
        ext in "(code|balance|sock)",
        ff in (0u16..(1 << 4)).prop_map(|b| FdFlags::from_bits(b << 1).unwrap()), // no append
    ) {
        path.push(ext);
        prop_assert!(
            create_bcfs().open(
                None,
                &path,
                OpenFlags::empty(),
                ff,
            ).is_ok()
        );
    }

    #[test]
    fn open_svc_fail(
        mut path in prop::bool::ANY.prop_map(|abs| if abs { good_path() } else { PathBuf::new() }),
        ext in "(code|balance|sock)",
        of in (0u16..(1 << 4)).prop_map(|b| OpenFlags::from_bits(b | 1).unwrap()),
        ff in (0u16..(1 << 5)).prop_map(|b| FdFlags::from_bits(b | 1).unwrap()),
    ) {
        path.push(ext);
        prop_assert!(create_bcfs().open(None, &path, of, ff,).is_err());
    }
}
