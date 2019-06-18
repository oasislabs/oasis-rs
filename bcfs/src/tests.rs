#![cfg(test)]

use std::{
    borrow::Cow,
    collections::HashMap,
    io::{IoSlice, IoSliceMut},
    path::PathBuf,
};

use blockchain_traits::{Blockchain, TransactionOutcome};
use mantle_types::{AccountMeta, Address};
use memchain::{Account, Memchain};
use wasi_types::{ErrNo, Fd, FdFlags, OpenFlags, Whence};

use crate::BCFS;

const ADDR_1: Address = Address([1u8; 20]);
const ADDR_2: Address = Address([2u8; 20]);
const BASE_GAS: u64 = 2100;
const GAS_PRICE: u64 = 0;
const CHAIN_NAME: &str = "testchain";

fn giga(val: u64) -> u64 {
    val * 1_000_000_000
}

fn create_memchain(
    mains: Vec<Option<memchain::AccountMain>>,
) -> impl Blockchain<Address = Address, AccountMeta = AccountMeta> {
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

    Memchain::new(CHAIN_NAME, genesis_state, BASE_GAS)
}

/// Returns a known-good home directory.
fn good_home() -> PathBuf {
    let mut p = PathBuf::from("/opt");
    p.push(CHAIN_NAME);
    p.push(hex::encode(&ADDR_2));
    p
}

macro_rules! testcase {
    (fn $fn_name:ident ( $ptx:ident : &mut dyn PendingTransaction ) -> u16 $body:block) => {
        #[test]
        fn $fn_name() {
            extern "C" fn test_main(ptxp: memchain::PtxPtr) -> u16 {
                let $ptx = unsafe { &mut **ptxp };
                $body
            }
            let mut bc = create_memchain(vec![None, Some(test_main)]);
            let receipt = bc.last_block_mut().transact(
                ADDR_1, ADDR_2, ADDR_1, /* payer */
                42,     /* value */
                b"input", BASE_GAS, GAS_PRICE,
            );
            assert_eq!(receipt.outcome(), TransactionOutcome::Success);
        }
    };
}

testcase!(
    fn close_fd(ptx: &mut dyn PendingTransaction) -> u16 {
        let mut bcfs = BCFS::new(ptx, CHAIN_NAME);
        for fd in 0u32..=3 {
            let fd = Fd::from(fd);
            assert!(bcfs.close(ptx, fd).is_ok());
            assert_eq!(bcfs.close(ptx, fd), Err(ErrNo::BadF)); // double close
        }
        for fd in 4u32..10 {
            assert_eq!(bcfs.close(ptx, Fd::from(fd)), Err(ErrNo::BadF));
        }
        0
    }
);

testcase!(
    fn open_close(ptx: &mut dyn PendingTransaction) -> u16 {
        let mut bcfs = BCFS::new(ptx, CHAIN_NAME);
        let mut abspath = good_home();
        abspath.push("somefile");
        let relpath = PathBuf::from("somefile");

        let abs_fd = bcfs
            .open(ptx, None, &abspath, OpenFlags::CREATE, FdFlags::empty())
            .unwrap();

        // double create
        assert_eq!(
            bcfs.open(ptx, None, &abspath, OpenFlags::EXCL, FdFlags::empty()),
            Err(ErrNo::Exist)
        );

        let abs_fd2 = bcfs
            .open(ptx, None, &abspath, OpenFlags::empty(), FdFlags::empty())
            .unwrap();
        let rel_fd = bcfs
            .open(ptx, None, &relpath, OpenFlags::empty(), FdFlags::APPEND)
            .unwrap();

        assert!(bcfs.close(ptx, abs_fd).is_ok());
        assert!(bcfs.close(ptx, abs_fd2).is_ok());
        assert!(bcfs.close(ptx, rel_fd).is_ok());
        0
    }
);

testcase!(
    fn read_write_basic(ptx: &mut dyn PendingTransaction) -> u16 {
        let mut bcfs = BCFS::new(ptx, CHAIN_NAME);

        let path = PathBuf::from("somefile");

        let fd = bcfs
            .open(ptx, None, &path, OpenFlags::CREATE, FdFlags::empty())
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
                        ptx,
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
                ptx,
                fd,
                &write_bufs
                    .iter()
                    .map(|b| IoSlice::new(b.as_bytes()))
                    .collect::<Vec<_>>()
            ),
            Ok(nbytes)
        );
        assert_read!(read_bufs, write_bufs, 0);

        assert_eq!(bcfs.seek(ptx, fd, 0, Whence::Start), Ok(0));
        assert_read!(read_bufs, write_bufs, nbytes);
        assert_read!(read_bufs, write_bufs, 0);

        assert_eq!(bcfs.seek(ptx, fd, -(nbytes as i64), Whence::End), Ok(0));
        assert_read!(read_bufs, write_bufs, nbytes);
        assert_read!(read_bufs, write_bufs, 0);

        assert_eq!(
            bcfs.seek(ptx, fd, -(nbytes as i64 - 2), Whence::Current),
            Ok(2)
        );
        assert_eq!(bcfs.seek(ptx, fd, -2, Whence::Current), Ok(0));

        assert_eq!(bcfs.seek(ptx, fd, 0, Whence::End), Ok(nbytes as u64));
        let write_bufs = ["hello", "blockchain"];
        let mut read_bufs = write_bufs
            .iter()
            .map(|b| vec![0u8; b.len()])
            .collect::<Vec<_>>();
        let new_nbytes = write_bufs.iter().map(|b| b.len()).sum();
        assert_eq!(
            bcfs.pwrite_vectored(
                ptx,
                fd,
                &write_bufs
                    .iter()
                    .map(|b| IoSlice::new(b.as_bytes()))
                    .collect::<Vec<_>>(),
                0
            ),
            Ok(new_nbytes)
        );
        assert_eq!(bcfs.tell(ptx, fd), Ok(nbytes as u64));
        assert_eq!(
            bcfs.pread_vectored(
                ptx,
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
        0
    }
);

testcase!(
    fn write_consecutive(ptx: &mut dyn PendingTransaction) -> u16 {
        let mut bcfs = BCFS::new(ptx, CHAIN_NAME);

        let path = PathBuf::from("somefile");

        let fd = bcfs
            .open(ptx, None, &path, OpenFlags::CREATE, FdFlags::empty())
            .unwrap();

        let write_bufs: &[&[u8]] = &[b"hello", b" world"];

        let nbytes = write_bufs.iter().map(|buf| buf.len()).sum();

        let mut read_buf = vec![0u8; nbytes];

        for wb in write_bufs {
            assert_eq!(
                bcfs.write_vectored(ptx, fd, &[IoSlice::new(wb)]),
                Ok(wb.len())
            );
        }

        assert_eq!(
            bcfs.seek(ptx, fd, -(nbytes as i64) + 1, Whence::Current),
            Ok(1)
        );

        assert_eq!(
            bcfs.read_vectored(ptx, fd, &mut [IoSliceMut::new(&mut read_buf)]),
            Ok(nbytes - 1)
        );

        assert_eq!(
            read_buf[..nbytes - 1],
            write_bufs
                .iter()
                .flat_map(|buf| buf.iter().cloned())
                .collect::<Vec<u8>>()[1..]
        );
        assert_eq!(read_buf[read_buf.len() - 1], 0);

        0
    }
);

testcase!(
    fn read_write_aliased(ptx: &mut dyn PendingTransaction) -> u16 {
        let mut bcfs = BCFS::new(ptx, CHAIN_NAME);

        let path = PathBuf::from("somefile");
        let abspath = good_home().join(&path);

        let abs_fd = bcfs
            .open(ptx, None, &path, OpenFlags::CREATE, FdFlags::empty())
            .unwrap();
        let rel_fd = bcfs
            .open(ptx, None, &abspath, OpenFlags::empty(), FdFlags::empty())
            .unwrap();

        let write_bufs = ["hello", "world"];
        let mut read_bufs = write_bufs
            .iter()
            .map(|b| vec![0u8; b.len()])
            .collect::<Vec<_>>();
        let nbytes = write_bufs.iter().map(|b| b.len()).sum();

        assert_eq!(
            bcfs.write_vectored(
                ptx,
                abs_fd, // NB: absolute path fd
                &write_bufs
                    .iter()
                    .map(|b| IoSlice::new(b.as_bytes()))
                    .collect::<Vec<_>>()
            ),
            Ok(nbytes)
        );
        bcfs.flush(ptx, abs_fd).unwrap();
        assert_eq!(
            bcfs.read_vectored(
                ptx,
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
        0
    }
);

testcase!(
    fn badf(ptx: &mut dyn PendingTransaction) -> u16 {
        let mut bcfs = BCFS::new(ptx, CHAIN_NAME);
        let badf = Fd::from(99u32);

        assert_eq!(
            bcfs.read_vectored(ptx, badf, &mut Vec::new()),
            Err(ErrNo::BadF)
        );

        assert_eq!(
            bcfs.write_vectored(ptx, badf, &Vec::new()),
            Err(ErrNo::BadF)
        );

        assert_eq!(
            bcfs.pread_vectored(ptx, badf, &mut Vec::new(), 0),
            Err(ErrNo::BadF)
        );

        assert_eq!(
            bcfs.pwrite_vectored(ptx, badf, &Vec::new(), 0),
            Err(ErrNo::BadF)
        );

        assert_eq!(bcfs.seek(ptx, badf, 0, Whence::Start), Err(ErrNo::BadF));

        assert_eq!(bcfs.fdstat(ptx, badf).unwrap_err(), ErrNo::BadF);
        assert_eq!(bcfs.filestat(ptx, badf).unwrap_err(), ErrNo::BadF);
        assert_eq!(bcfs.tell(ptx, badf).unwrap_err(), ErrNo::BadF);
        assert_eq!(bcfs.renumber(ptx, badf, badf).unwrap_err(), ErrNo::BadF);

        assert_eq!(bcfs.close(ptx, badf), Err(ErrNo::BadF));
        0
    }
);

testcase!(
    fn renumber(ptx: &mut dyn PendingTransaction) -> u16 {
        let mut bcfs = BCFS::new(ptx, CHAIN_NAME);
        let somefile = PathBuf::from("somefile");
        let anotherfile = PathBuf::from("anotherfile");

        let somefile_fd = bcfs
            .open(ptx, None, &somefile, OpenFlags::CREATE, FdFlags::empty())
            .unwrap();
        let anotherfile_fd = bcfs
            .open(ptx, None, &anotherfile, OpenFlags::CREATE, FdFlags::empty())
            .unwrap();

        let write_bufs = ["destination", "somefile"];
        bcfs.write_vectored(
            ptx,
            somefile_fd,
            &write_bufs
                .iter()
                .map(|b| IoSlice::new(b.as_bytes()))
                .collect::<Vec<_>>(),
        )
        .unwrap();

        bcfs.renumber(ptx, somefile_fd, anotherfile_fd).unwrap();

        assert_eq!(
            bcfs.read_vectored(ptx, somefile_fd, &mut Vec::new()),
            Err(ErrNo::BadF)
        );

        let mut read_buf = vec![0u8; 1];
        assert_eq!(
            bcfs.pread_vectored(
                ptx,
                anotherfile_fd,
                &mut [IoSliceMut::new(&mut read_buf)],
                0,
            ),
            Ok(read_buf.len())
        );
        assert_eq!(
            bcfs.tell(ptx, anotherfile_fd),
            Ok(write_bufs.iter().map(|b| b.len() as u64).sum())
        );
        0
    }
);
