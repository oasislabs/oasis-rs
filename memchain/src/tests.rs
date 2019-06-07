#![cfg(test)]

use crate::*;

const ADDR_1: Address = Address([1u8; 20]);
const ADDR_2: Address = Address([2u8; 20]);

fn giga(num: u64) -> u64 {
    num * 1_000_000_000
}

extern "C" fn nop_main(_bc: *const *mut dyn Blockchain<Address = Address>) -> u16 {
    0
}

extern "C" fn simple_main(bc: *const *mut dyn Blockchain<Address = Address>) -> u16 {
    let bc = unsafe { &mut **bc };

    assert_eq!(bc.sender(), &ADDR_2);

    bc.emit(vec![[42u8; 32]], vec![0u8; 3]);

    let mut rv = bc.fetch_input();
    rv.push(4);
    bc.ret(rv);

    0
}

extern "C" fn fail_main(bc: *const *mut dyn Blockchain<Address = Address>) -> u16 {
    let bc = unsafe { &mut **bc };
    bc.err(r"¯\_(ツ)_/¯".as_bytes().to_vec());
    1
}

extern "C" fn subtx_main(bc: *const *mut dyn Blockchain<Address = Address>) -> u16 {
    let bc = unsafe { &mut **bc };
    bc.transact(
        Address::default(), /* caller */
        ADDR_1,             /* callee */
        0,
        bc.fetch_input(),
        1_000_000,
        0,
    );

    bc.set(
        &Address::default(),
        b"common_key".to_vec(),
        b"uncommon_value".to_vec(),
    )
    .unwrap();

    let mut rv = bc.fetch_ret();
    rv.push(5);
    bc.ret(rv);
    0
}

fn create_bc<'bc>(
    mains: Vec<Option<extern "C" fn(*const *mut dyn Blockchain<Address = Address>) -> u16>>,
) -> Memchain<'bc> {
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

    Memchain::new("memchain".to_string(), genesis_state)
}

#[test]
fn transfer() {
    let mut bc = create_bc(vec![None, Some(nop_main)]);
    assert_eq!(bc.metadata_at(&ADDR_1).unwrap().balance, giga(1));
    assert_eq!(bc.metadata_at(&ADDR_2).unwrap().balance, giga(2));
    let value = 50;
    bc.transact(ADDR_1, ADDR_2, value, Vec::new(), BASE_GAS, 1);
    assert_eq!(
        bc.metadata_at(&ADDR_1).unwrap().balance,
        giga(1) - BASE_GAS - value,
    );
    assert_eq!(bc.metadata_at(&ADDR_2).unwrap().balance, giga(2) + value,);
}

#[test]
fn static_account() {
    let mut bc = create_bc(vec![None, None]);

    bc.create_block(); // should take state from prev block

    let addr1 = ADDR_1;
    let addr2 = ADDR_2;

    assert_eq!(bc.metadata_at(&addr1).unwrap().balance, giga(1),);

    assert_eq!(bc.metadata_at(&addr2).unwrap().balance, giga(2),);

    let code2 = "\0asm not wasm 2".as_bytes();
    assert_eq!(bc.code_at(&addr2).unwrap(), code2);
    assert_eq!(bc.code_len(&addr2), code2.len() as u32);

    let common_key = b"common_key".as_ref();
    assert_eq!(
        bc.get(&addr1, common_key),
        Ok(Some(b"common_value".as_ref()))
    );
    assert_eq!(
        bc.get(&addr2, common_key),
        Ok(Some(b"common_value".as_ref()))
    );

    assert_eq!(bc.get(&addr1, b"key_1"), Ok(Some(b"value_1".as_ref())));

    assert_eq!(
        bc.get(&Address::default(), common_key),
        Err(KVError::NoAccount)
    );
    assert!(bc.get(&addr1, &Vec::new()).unwrap().is_none());
}

#[test]
fn simple_tx() {
    let mut bc = create_bc(vec![Some(simple_main), None]);
    bc.transact(ADDR_2, ADDR_1, 50, vec![1, 2, 3], BASE_GAS, 0);
    assert_eq!(bc.fetch_ret(), vec![1, 2, 3, 4]);
}

#[test]
fn revert_tx() {
    let mut bc = create_bc(vec![None, Some(fail_main)]);
    bc.transact(ADDR_1, ADDR_2, 10_000, Vec::new(), BASE_GAS, 1);
    assert_eq!(bc.metadata_at(&ADDR_1).unwrap().balance, giga(1) - BASE_GAS,);
    assert_eq!(bc.metadata_at(&ADDR_2).unwrap().balance, giga(2),);
}

#[test]
fn subtx_ok() {
    let mut bc = create_bc(vec![Some(simple_main), Some(subtx_main)]);
    bc.transact(ADDR_1, ADDR_2, 1000, vec![1, 2, 3], BASE_GAS, 0);

    assert_eq!(bc.fetch_ret(), vec![1, 2, 3, 4, 5]);

    let logs = bc.last_block().logs();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].topics, vec![[42u8; 32]]);
    assert_eq!(logs[0].data, vec![0u8; 3]);

    assert_eq!(
        bc.get(&ADDR_2, b"common_key"),
        Ok(Some(b"uncommon_value".as_ref()))
    );
}

#[test]
fn subtx_revert() {
    let mut bc = create_bc(vec![Some(fail_main), Some(subtx_main)]);
    bc.transact(ADDR_1, ADDR_2, 0, vec![1, 2, 3], BASE_GAS, 0);
    assert_eq!(bc.fetch_ret(), vec![]);
    assert!(bc.last_block().logs().is_empty());
    assert_eq!(
        bc.get(&ADDR_2, b"common_key"),
        Ok(Some(b"common_value".as_ref()))
    );
}
