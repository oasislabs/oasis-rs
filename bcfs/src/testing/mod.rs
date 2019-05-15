pub mod ffi;

use oasis_types::Address;

use crate::{AccountMetadata, BlockchainIntrinsics, KVStore};

impl<'bc> KVStore for memchain::Blockchain<'bc> {
    fn contains(&self, key: &[u8]) -> bool {
        self.current_tx()
            .and_then(memchain::Transaction::current_account)
            .map(|acct| acct.storage.contains_key(key))
            .unwrap_or(false)
    }

    fn size(&self, key: &[u8]) -> u64 {
        self.get(key).map(|v| v.len() as u64).unwrap_or(0)
    }

    fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.current_tx()
            .and_then(memchain::Transaction::current_account)
            .and_then(|acct| acct.storage.get(key))
            .map(Vec::as_slice)
    }

    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.with_current_tx(move |tx| {
            if let Some(acct) = tx.current_account_mut() {
                acct.storage.insert(key, value);
            }
        });
    }
}

impl<'b> BlockchainIntrinsics for memchain::Blockchain<'b> {
    fn input(&self) -> Vec<u8> {
        self.current_tx()
            .map(|tx| tx.current_frame().input.to_vec())
            .unwrap_or_default()
    }

    fn input_len(&self) -> u64 {
        self.current_tx()
            .map(|tx| tx.current_frame().input.len() as u64)
            .unwrap_or_default()
    }

    fn ret(&mut self, mut data: Vec<u8>) {
        self.with_current_tx(|tx| tx.current_frame_mut().ret_buf.append(&mut data));
    }

    fn ret_err(&mut self, mut data: Vec<u8>) {
        self.with_current_tx(|tx| tx.current_frame_mut().err_buf.append(&mut data));
    }

    fn emit(&mut self, topics: Vec<[u8; 32]>, data: Vec<u8>) {
        self.with_current_tx(|tx| tx.log(topics, data));
    }

    fn code_at(&self, addr: &Address) -> Option<&[u8]> {
        self.current_state()
            .get(addr)
            .map(|acct| acct.code.as_slice())
    }

    fn code_len(&self, addr: &Address) -> u64 {
        self.current_state()
            .get(addr)
            .map(|acct| acct.code.len() as u64)
            .unwrap_or_default()
    }

    fn metadata_at(&self, addr: &Address) -> Option<AccountMetadata> {
        self.current_state().get(addr).map(|acct| AccountMetadata {
            balance: acct.balance,
            expiry: acct.expiry,
        })
    }
}
