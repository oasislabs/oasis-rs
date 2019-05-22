#![cfg(test)]

use crate::*;

fn giga(num: u64) -> u64 {
    num * 1_000_000_000
}

extern "C" fn nop_main(_bc: *mut dyn Blockchain<Address>) -> u16 {
    0
}

extern "C" fn simple_main(bc: *mut dyn Blockchain<Address>) -> u16 {
    let bc = unsafe { &mut *bc };

    assert_eq!(bc.sender(), &Address::from(2));

    bc.emit(vec![[42u8; 32]], vec![0u8; 3]);

    let mut rv = bc.fetch_input();
    rv.push(4);
    bc.ret(rv);

    0
}

extern "C" fn fail_main(bc: *mut dyn Blockchain<Address>) -> u16 {
    let bc = unsafe { &mut *bc };
    bc.err(r"¯\_(ツ)_/¯".as_bytes().to_vec());
    1
}

extern "C" fn subtx_main(bc: *mut dyn Blockchain<Address>) -> u16 {
    let bc = unsafe { &mut *bc };
    bc.transact(
        Address::default(), /* caller */
        Address::from(1),   /* callee */
        0,
        bc.fetch_input(),
        1_000_000,
        0,
    );

    bc.set(
        &Address::default(),
        b"common_key".to_vec(),
        b"uncommon_value".to_vec(),
    );

    let mut rv = bc.fetch_ret();
    rv.push(5);
    bc.ret(rv);
    0
}

fn create_bc<'bc>(
    mains: Vec<Option<extern "C" fn(*mut dyn Blockchain<Address>) -> u16>>,
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

    Memchain::new(genesis_state)
}

#[test]
fn transfer() {
    let bc = create_bc(vec![None, Some(nop_main)]);
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(1)).unwrap().balance,
        giga(1)
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(2)).unwrap().balance,
        giga(2)
    );
    let value = 50;
    bc.borrow_mut().transact(
        Address::from(1),
        Address::from(2),
        value,
        Vec::new(),
        BASE_GAS,
        1,
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(1)).unwrap().balance,
        giga(1) - BASE_GAS - value,
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(2)).unwrap().balance,
        giga(2) + value,
    );
}

#[test]
fn static_account() {
    let bc = create_bc(vec![None, None]);

    bc.borrow_mut().create_block(); // should take state from prev block

    let addr1 = Address::from(1);
    let addr2 = Address::from(2);

    assert_eq!(bc.borrow().metadata_at(&addr1).unwrap().balance, giga(1),);

    assert_eq!(bc.borrow().metadata_at(&addr2).unwrap().balance, giga(2),);

    let code2 = "\0asm not wasm 2".as_bytes();
    assert_eq!(bc.borrow().code_at(&addr2).unwrap(), code2);
    assert_eq!(bc.borrow().code_len(&addr2), code2.len() as u64);

    let common_key = b"common_key".as_ref();
    assert_eq!(
        bc.borrow().get(&addr1, common_key),
        Some(b"common_value".as_ref())
    );
    assert_eq!(
        bc.borrow().get(&addr2, common_key),
        Some(b"common_value".as_ref())
    );

    assert_eq!(bc.borrow().get(&addr1, b"key_1"), Some(b"value_1".as_ref()));

    assert!(bc.borrow().get(&Address::zero(), common_key).is_none());
    assert!(bc.borrow().get(&addr1, &Vec::new()).is_none());
}

#[test]
fn simple_tx() {
    let bc = create_bc(vec![Some(simple_main), None]);
    bc.borrow_mut().transact(
        Address::from(2),
        Address::from(1),
        50,
        vec![1, 2, 3],
        BASE_GAS,
        0,
    );
    assert_eq!(bc.borrow().fetch_ret(), vec![1, 2, 3, 4]);
}

#[test]
fn revert_tx() {
    let bc = create_bc(vec![None, Some(fail_main)]);
    bc.borrow_mut().transact(
        Address::from(1),
        Address::from(2),
        10_000,
        Vec::new(),
        BASE_GAS,
        1,
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(1)).unwrap().balance,
        giga(1) - BASE_GAS,
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(2)).unwrap().balance,
        giga(2),
    );
}

#[test]
fn subtx_ok() {
    let bc = create_bc(vec![Some(simple_main), Some(subtx_main)]);
    bc.borrow_mut().transact(
        Address::from(1),
        Address::from(2),
        1000,
        vec![1, 2, 3],
        BASE_GAS,
        0,
    );

    let bc_ref = bc.borrow();
    assert_eq!(bc_ref.fetch_ret(), vec![1, 2, 3, 4, 5]);

    let logs = bc_ref.last_block().logs();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].topics, vec![[42u8; 32]]);
    assert_eq!(logs[0].data, vec![0u8; 3]);

    assert_eq!(
        bc_ref.get(&Address::from(2), b"common_key"),
        Some(b"uncommon_value".as_ref())
    );
}

#[test]
fn subtx_revert() {
    let bc = create_bc(vec![Some(fail_main), Some(subtx_main)]);
    bc.borrow_mut().transact(
        Address::from(1),
        Address::from(2),
        0,
        vec![1, 2, 3],
        BASE_GAS,
        0,
    );
    let bc_ref = bc.borrow();
    assert_eq!(bc_ref.fetch_ret(), vec![]);
    assert!(bc_ref.last_block().logs().is_empty());
    assert_eq!(
        bc_ref.get(&Address::from(2), b"common_key"),
        Some(b"common_value".as_ref())
    );
}
