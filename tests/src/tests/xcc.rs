use oasis_std::{abi_encode, Address, Context};

use crate::mock_gateway::{DeployCall, GatewayHandlers, MockGateway, RpcCall};

static SERVICE_A_BYTECODE: &[u8] = include_bytes!("../../../target/wasm32-wasi/release/a.wasm");

#[test]
fn test_import() {
    super::test_oasis_interface("a", "ServiceA");
}

#[test]
fn test_native_client() {
    let a_addr = Address([1u8; 20]);
    let b_addr = Address([2u8; 20]);
    let deploy_message = "message";
    let numbers = vec![b::Number(99), b::Number(1)];
    let rpc_return = numbers.clone();

    let encoded_deploy_message = abi_encode!(deploy_message).unwrap();
    let expected_initcode = SERVICE_A_BYTECODE.iter().chain(&encoded_deploy_message);

    let func_idx = 0u8;
    let expected_rpc_payload = abi_encode!(func_idx, b_addr).unwrap();

    let gateway = MockGateway::new(GatewayHandlers {
        deploy: box move |_| Ok(a_addr),
        rpc: box move |_, _| Ok(abi_encode!(rpc_return).unwrap()),
    });

    let client = a::ServiceAClient::deploy(&gateway, &Context::default(), deploy_message).unwrap();

    assert!(gateway.rpcs.borrow().is_empty());

    {
        let deploys = gateway.deploys.borrow();
        assert_eq!(deploys.len(), 1);
        let DeployCall { initcode, .. } = &deploys[0];
        assert!(initcode.iter().eq(expected_initcode));
    }

    let rpc_output = client.call_b(&Context::default(), b_addr).unwrap();
    assert_eq!(rpc_output.unwrap(), numbers);

    assert_eq!(gateway.deploys.borrow().len(), 1);

    {
        let rpcs = gateway.rpcs.borrow();
        assert_eq!(rpcs.len(), 1);
        let RpcCall {
            callee, payload, ..
        } = &rpcs[0];
        assert_eq!(*callee, a_addr);
        assert_eq!(payload, &expected_rpc_payload);
    }
}
