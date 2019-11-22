use std::cell::RefCell;

use oasis_std::{Address, RpcError};

pub struct MockGateway {
    pub handlers: GatewayHandlers,
    pub deploys: RefCell<Vec<DeployCall>>,
    pub rpcs: RefCell<Vec<RpcCall>>,
}

pub struct DeployCall {
    pub initcode: Vec<u8>,
    pub outcome: Result<Address, RpcError>,
}

pub struct RpcCall {
    pub callee: Address,
    pub payload: Vec<u8>,
    pub outcome: Result<Vec<u8>, RpcError>,
}

impl MockGateway {
    pub fn new(handlers: GatewayHandlers) -> Self {
        Self {
            handlers,
            deploys: RefCell::new(Vec::new()),
            rpcs: RefCell::new(Vec::new()),
        }
    }
}

impl oasis_client::gateway::Gateway for MockGateway {
    fn deploy(&self, initcode: &[u8]) -> Result<Address, RpcError> {
        let outcome = (self.handlers.deploy)(initcode);
        self.deploys.borrow_mut().push(DeployCall {
            initcode: initcode.to_vec(),
            outcome: outcome.clone(),
        });
        outcome
    }

    fn rpc(&self, address: Address, payload: &[u8]) -> Result<Vec<u8>, RpcError> {
        let outcome = (self.handlers.rpc)(address, payload);
        self.rpcs.borrow_mut().push(RpcCall {
            callee: address,
            payload: payload.to_vec(),
            outcome: outcome.clone(),
        });
        outcome
    }
}

pub struct GatewayHandlers {
    pub deploy: Box<dyn Fn(&[u8]) -> Result<Address, RpcError>>,
    #[allow(clippy::type_complexity)]
    pub rpc: Box<dyn Fn(Address, &[u8]) -> Result<Vec<u8>, RpcError>>,
}
