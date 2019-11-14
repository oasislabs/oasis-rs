const API_KEY: &str = "AAACL7PMQhh3/rxLr9KJpsAJhz5zBlpAB73uwgAt/6BQ4+Bw";
const GATEWAY_URL: &str = "http://localhost:1234";
//^ The tests assume that a developer gateway is listening on this address.

const FIXTURE_BYTECODE: &[u8] = include_bytes!("../../target/service/simple-wasi.wasm");

#[test]
fn test_gateway_e2e() {
    let gateway = oasis_client::HttpGatewayBuilder::new(GATEWAY_URL)
        .api_key(API_KEY)
        .build();

    let mut initcode = FIXTURE_BYTECODE.to_vec();
    initcode.extend(b"pong");

    let fixture_addr = gateway.deploy(&initcode).unwrap();
    let res = gateway.rpc(fixture_addr, b"hello, service!").unwrap();
    assert_eq!(
        String::from_utf8(res).unwrap(),
        "hello, client! your initial data was pong"
    );
}
