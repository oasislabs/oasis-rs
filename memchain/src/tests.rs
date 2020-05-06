#![cfg(test)]

use blockchain_traits::PendingTransaction;

use crate::*;

const ADDR_1: Address = Address([1u8; 20]);
const ADDR_2: Address = Address([2u8; 20]);

const BASE_GAS: u64 = 2100;

fn giga(num: u128) -> u128 {
    num * 1_000_000_000
}

extern "C" fn nop_main(_ptx: *const *mut dyn PendingTransaction) -> u16 {
    0
}

extern "C" fn simple_main(ptx: *const *mut dyn PendingTransaction) -> u16 {
    let ptx = unsafe { &mut **ptx };

    assert_eq!(ptx.sender(), &ADDR_2);

    ptx.emit(vec![[42u8; 32].as_ref()].as_slice(), &[0u8; 3]);

    let mut rv = ptx.input().to_vec();
    rv.push(4);
    ptx.ret(&rv);

    0
}

extern "C" fn fail_main(ptx: *const *mut dyn PendingTransaction) -> u16 {
    let ptx = unsafe { &mut **ptx };
    ptx.err(r"¯\_(ツ)_/¯".as_bytes());
    1
}

extern "C" fn subtx_main(ptx: *const *mut dyn PendingTransaction) -> u16 {
    let ptx = unsafe { &mut **ptx };
    let subtx = ptx.transact(ADDR_1, 0 /* value */, &ptx.input().to_vec());

    if subtx.reverted() {
        ptx.ret(b"error");
        return 1;
    }

    ptx.state_mut().set(b"common_key", b"uncommon_value");

    let mut rv = subtx.output().to_vec();
    rv.push(5);
    ptx.ret(&rv);
    0
}

fn create_bc<'bc>(
    mains: Vec<Option<extern "C" fn(*const *mut dyn PendingTransaction) -> u16>>,
) -> Memchain<'bc> {
    let genesis_state = mains
        .into_iter()
        .enumerate()
        .map(|(i, main)| {
            let i = i + 1;
            (
                Address([i as u8; 20]),
                Cow::Owned(Account {
                    balance: giga(i as u128),
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

    Memchain::new("memchain".to_string(), genesis_state, BASE_GAS)
}

#[test]
fn transfer() {
    let mut bc = create_bc(vec![None, Some(nop_main)]);
    assert_eq!(
        bc.last_block().account_meta_at(&ADDR_1).unwrap().balance,
        giga(1)
    );
    assert_eq!(
        bc.last_block().account_meta_at(&ADDR_2).unwrap().balance,
        giga(2)
    );
    let value = 50;
    bc.last_block_mut()
        .transact(ADDR_1, ADDR_2, ADDR_1, value, &Vec::new(), BASE_GAS, 1);
    assert_eq!(
        bc.last_block().account_meta_at(&ADDR_1).unwrap().balance,
        giga(1) - u128::from(BASE_GAS) - value,
    );
    assert_eq!(
        bc.last_block().account_meta_at(&ADDR_2).unwrap().balance,
        giga(2) + value,
    );
}

#[test]
fn static_account() {
    let mut bc = create_bc(vec![None, None]);

    bc.create_block(); // should take state from prev block

    assert_eq!(
        bc.last_block().account_meta_at(&ADDR_1).unwrap().balance,
        giga(1),
    );

    assert_eq!(
        bc.last_block().account_meta_at(&ADDR_2).unwrap().balance,
        giga(2),
    );

    let code2 = b"\0asm not wasm 2".as_ref();
    assert_eq!(bc.last_block().code_at(&ADDR_2).unwrap(), code2);

    let common_key = b"common_key".as_ref();
    assert_eq!(
        bc.last_block().state_at(&ADDR_1).unwrap().get(common_key),
        Some(b"common_value".to_vec())
    );
    assert_eq!(
        bc.last_block().state_at(&ADDR_2).unwrap().get(common_key),
        Some(b"common_value".to_vec())
    );

    assert_eq!(
        bc.block(1)
            .unwrap()
            .state_at(&ADDR_1)
            .unwrap()
            .get(b"key_1"),
        Some(b"value_1".to_vec())
    );

    assert!(bc.last_block().state_at(&Address::default()).is_none());
    assert_eq!(
        bc.last_block().state_at(&ADDR_1).unwrap().get(&Vec::new()),
        None
    );
}

#[test]
fn simple_tx() {
    let mut bc = create_bc(vec![Some(simple_main), None]);
    bc.last_block_mut()
        .transact(ADDR_2, ADDR_1, ADDR_1, 50, &[1u8, 2, 3], BASE_GAS, 0);
    assert_eq!(
        bc.last_block().receipts().last().unwrap().output(),
        &[1u8, 2, 3, 4]
    );
}

#[test]
fn revert_tx() {
    let mut bc = create_bc(vec![None, Some(fail_main)]);
    bc.last_block_mut()
        .transact(ADDR_1, ADDR_2, ADDR_2, 10_000, &Vec::new(), BASE_GAS, 1);
    assert_eq!(
        bc.last_block().account_meta_at(&ADDR_2).unwrap().balance,
        giga(2) - u128::from(BASE_GAS),
    );
    assert_eq!(
        bc.last_block().account_meta_at(&ADDR_1).unwrap().balance,
        giga(1),
    );
}

#[test]
fn subtx_ok() {
    let mut bc = create_bc(vec![Some(simple_main), Some(subtx_main)]);
    let receipt =
        bc.last_block_mut()
            .transact(ADDR_1, ADDR_2, ADDR_2, 1000, &[1, 2, 3], BASE_GAS * 2, 0);

    assert_eq!(
        receipt.outcome(),
        blockchain_traits::TransactionOutcome::Success
    );

    assert_eq!(
        bc.last_block().receipts().last().unwrap().output(),
        &[1, 2, 3, 4, 5]
    );

    let events = bc.last_block().events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].topics, vec![[42u8; 32]]);
    assert_eq!(events[0].data, &[0, 0, 0]);

    assert_eq!(
        bc.last_block()
            .state_at(&ADDR_2)
            .unwrap()
            .get(b"common_key"),
        Some(b"uncommon_value".to_vec())
    );
}

#[test]
fn subtx_revert() {
    let mut bc = create_bc(vec![Some(fail_main), Some(subtx_main)]);
    bc.last_block_mut()
        .transact(ADDR_1, ADDR_2, ADDR_2, 0, &[1, 2, 3], BASE_GAS, 0);
    assert_eq!(
        bc.last_block().receipts().last().unwrap().output(),
        b"error"
    );
    assert!(bc.last_block().events().is_empty());
    assert_eq!(
        bc.last_block()
            .state_at(&ADDR_2)
            .unwrap()
            .get(b"common_key"),
        Some(b"common_value".to_vec())
    );
}
