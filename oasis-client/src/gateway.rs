use std::{collections::HashMap, str::FromStr as _};

use anyhow::{anyhow, Result};
use http::{header::HeaderName, HeaderValue};
use oasis_types::Address;
use uuid::Uuid;

#[cfg(not(target_env = "sgx"))]
use reqwest::Client;

use crate::{api::*, polling::PollingService};

/// Holds necessary information to make http requests to the gateway.
///
/// # Example
///
/// ```no_run
/// let url = "https://gateway.devnet.oasiscloud.io";
/// let api_key = "AAACL7PMQhh3/rxLr9KJpsAJhz5zBlpAB73uwgAt/6BQ4+Bw";
/// let gateway = oasis_client::HttpGatewayBuilder::new(url)
///     .api_key(api_key)
///     .build();
/// let address = gateway.deploy(b"service Wasm bytecode").unwrap();
/// let response = gateway.rpc(address, b"data").unwrap();
/// ```
pub struct HttpGateway {
    /// The url of the gateway.
    url: String,

    /// The http headers to include in all requests sent to the gateway.
    headers: HashMap<String, String>,

    /// The http session used for sending http requests.
    client: Client,

    /// A polling service used to receive responses from asynchronous requests made.
    polling_service: PollingService,
}

#[derive(Clone, Debug)]
pub struct HttpGatewayBuilder {
    url: String,
    api_key: Option<String>,
    headers: HashMap<String, String>,
}

impl HttpGatewayBuilder {
    pub fn new(url: impl AsRef<str>) -> Self {
        Self {
            url: url.as_ref().to_string(),
            ..Default::default()
        }
    }

    pub fn api_key(mut self, api_key: impl AsRef<str>) -> Self {
        self.api_key = Some(api_key.as_ref().to_string());
        self
    }

    /// Assign the value of the named header.
    pub fn header(mut self, name: impl ToString, value: impl ToString) -> Self {
        self.headers.insert(name.to_string(), value.to_string());
        self
    }

    /// Assigns the provided headers as the defaults for all requests made by the `HttpGateway`.
    pub fn headers(mut self, headers: impl IntoIterator<Item = (String, String)>) -> Self {
        self.headers = headers.into_iter().collect();
        self
    }

    /// Creates a new `HttpGateway` instance that is configured with headers expected by the
    /// Oasis Developer gateway.
    pub fn build(self) -> HttpGateway {
        let session_key = Uuid::new_v4().to_string();

        let mut headers = self.headers;
        headers.insert("X-OASIS-INSECURE-AUTH".to_string(), "1".to_string());
        if let Some(api_key) = self.api_key {
            headers.insert("X-OASIS-LOGIN-TOKEN".to_string(), api_key);
        }
        headers.insert("X-OASIS-SESSION-KEY".to_string(), session_key);
        HttpGateway::new(
            self.url,
            headers,
            PollingService::new(500 /* sleep duration */, 20 /* max attempts */),
        )
    }
}

impl Default for HttpGatewayBuilder {
    fn default() -> Self {
        Self {
            url: "https://gateway.devnet.oasiscloud.io".to_string(),
            api_key: None,
            headers: HashMap::default(),
        }
    }
}

impl HttpGateway {
    /// Creates a new `HttpGateway` pointed at `url` and with default `headers`.
    pub fn new(
        url: String,
        headers: HashMap<String, String>,
        polling_service: PollingService,
    ) -> Self {
        Self {
            url,
            headers,
            client: Client::new(),
            polling_service,
        }
    }

    pub fn deploy(&self, initcode: &[u8]) -> Result<Address> {
        let initcode_hex = hex::encode(initcode);
        info!("deploying service `{}`", &initcode_hex[..32]);

        let body = GatewayRequest::Deploy {
            data: format!("0x{}", initcode_hex),
        };

        match &self.post_and_poll(SERVICE_DEPLOY_API, body)? {
            Event::DeployService { address, .. } => {
                Ok(Address::from_str(&address[2..] /* strip 0x */)?)
            }
            _ => Err(anyhow!("invalid event")),
        }
    }

    pub fn rpc(&self, address: Address, data: &[u8]) -> Result<Vec<u8>> {
        info!("making RPC to {}", address);

        let body = GatewayRequest::Execute {
            address: address.to_string(),
            data: format!("0x{}", hex::encode(data)),
        };

        match &self.post_and_poll(SERVICE_EXECUTE_API, body)? {
            Event::ExecuteService { output, .. } => Ok(hex::decode(&output[2..])?),
            e => Err(anyhow!("invalid event {:?}", e)),
        }
    }

    fn post_and_poll(&self, api: DeveloperGatewayApi, body: GatewayRequest) -> Result<Event> {
        let response: AsyncResponse = self.request(api.method, api.url, body)?;

        let event = self.polling_service.poll_for(self, response.id)?;
        if let Event::Error { description, .. } = event {
            Err(anyhow!("{}", description))
        } else {
            Ok(event)
        }
    }

    pub(crate) fn request<P: serde::Serialize, Q: serde::de::DeserializeOwned>(
        &self,
        method: RequestMethod,
        url: &str,
        payload: P,
    ) -> Result<Q> {
        let url = if self.url.ends_with('/') {
            format!("{}{}", self.url, url)
        } else {
            format!("{}/{}", self.url, url)
        };
        let builder = match method {
            RequestMethod::GET => self.client.get(&url),
            RequestMethod::POST => self.client.post(&url),
        };

        let header_map = self
            .headers
            .iter()
            .map(|(k, v)| {
                (
                    HeaderName::from_str(k).unwrap(),
                    HeaderValue::from_str(v).unwrap(),
                )
            })
            .collect();

        let mut res = builder.headers(header_map).json(&payload).send()?;
        if res.status().is_success() {
            Ok(res.json()?)
        } else {
            Err(anyhow!("gateway returned error: {}", res.status()))
        }
    }
}

#[cfg(all(test, not(target_env = "sgx")))]
mod tests {
    use super::*;

    use mockito::mock;
    use serde_json::json;

    // The following are randomly sampled from allowable strings.
    const API_KEY: &str = "AAACL7PMQhh3/rxLr9KJpsAJhz5zBlpAB73uwgAt/6BQ4+Bw";
    const PAYLOAD_HEX: &str = "0x144c6bda090723de712e52b92b4c758d78348ddce9aa80ca8ef51125bfb308";
    const FIXTURE_ADDR: &str = "0xb8b3666d8fea887d97ab54f571b8e5020c5c8b58";

    #[test]
    fn test_deploy() {
        let fixture_addr = Address::from_str(&FIXTURE_ADDR[2..]).unwrap();
        let poll_id = 42;

        let _m_deploy = mock("POST", "/v0/api/service/deploy")
            .match_header("content-type", "application/json")
            .match_header("x-oasis-login-token", API_KEY)
            .match_body(mockito::Matcher::Json(json!({ "data": PAYLOAD_HEX })))
            .with_header("content-type", "text/json")
            .with_body(json!({ "id": poll_id }).to_string())
            .create();

        let _m_poll = mock("POST", "/v0/api/service/poll")
            .match_header("content-type", "application/json")
            .match_header("x-oasis-login-token", API_KEY)
            .match_body(mockito::Matcher::Json(json!({
                "offset": poll_id,
                "count": 1,
                "discard_previous": true,
            })))
            .with_header("content-type", "text/json")
            .with_body(
                json!({
                    "offset": poll_id,
                    "events": [
                        { "id": poll_id, "address": FIXTURE_ADDR }
                    ]
                })
                .to_string(),
            )
            .create();

        let gateway = HttpGatewayBuilder::new(mockito::server_url())
            .api_key(API_KEY)
            .build();
        let addr = gateway
            .deploy(&hex::decode(&PAYLOAD_HEX[2..]).unwrap())
            .unwrap();

        assert_eq!(addr, fixture_addr);
    }

    #[test]
    fn test_rpc() {
        let fixture_addr = Address::from_str(&FIXTURE_ADDR[2..]).unwrap();
        let poll_id = 42;
        let expected_output = "hello, client!";
        let hex_output = "0x".to_string() + &hex::encode(expected_output.as_bytes());

        let _m_execute = mock("POST", "/v0/api/service/execute")
            .match_header("content-type", "application/json")
            .match_header("x-oasis-login-token", mockito::Matcher::Missing)
            .match_body(mockito::Matcher::Json(json!({
                "address": FIXTURE_ADDR,
                "data": PAYLOAD_HEX,
            })))
            .with_header("content-type", "text/json")
            .with_body(json!({ "id": poll_id }).to_string())
            .create();

        let _m_poll = mock("POST", "/v0/api/service/poll")
            .match_header("content-type", "application/json")
            .match_header("x-oasis-login-token", mockito::Matcher::Missing)
            .match_body(mockito::Matcher::Json(json!({
                "offset": poll_id,
                "count": 1,
                "discard_previous": true,
            })))
            .with_header("content-type", "text/json")
            .with_body(
                json!({
                    "offset": poll_id,
                    "events": [
                        { "id": poll_id, "address": FIXTURE_ADDR, "output": hex_output }
                    ]
                })
                .to_string(),
            )
            .create();

        let gateway = HttpGatewayBuilder::new(mockito::server_url()).build();
        let output = gateway
            .rpc(fixture_addr, &hex::decode(&PAYLOAD_HEX[2..]).unwrap())
            .unwrap();

        assert_eq!(output, expected_output.as_bytes());
    }

    #[test]
    fn test_error() {
        let fixture_addr = Address::from_str(&FIXTURE_ADDR[2..]).unwrap();
        let poll_id = 42;
        let err_code = 99;
        let err_msg = "error!";

        let _m_execute = mock("POST", "/v0/api/service/execute")
            .match_header("content-type", "application/json")
            .match_body(mockito::Matcher::Json(json!({
                "address": FIXTURE_ADDR,
                "data": PAYLOAD_HEX,
            })))
            .with_header("content-type", "text/json")
            .with_body(json!({ "id": poll_id }).to_string())
            .create();

        let _m_poll = mock("POST", "/v0/api/service/poll")
            .match_header("content-type", "application/json")
            .match_body(mockito::Matcher::Json(json!({
                "offset": poll_id,
                "count": 1,
                "discard_previous": true,
            })))
            .with_header("content-type", "text/json")
            .with_body(
                json!({
                    "offset": poll_id,
                    "events": [
                        { "id": poll_id, "error_code": err_code, "description": err_msg }
                    ]
                })
                .to_string(),
            )
            .create();

        let gateway = HttpGatewayBuilder::new(mockito::server_url())
            .api_key(API_KEY)
            .build();
        let err_output = gateway
            .rpc(fixture_addr, &hex::decode(&PAYLOAD_HEX[2..]).unwrap())
            .unwrap_err();

        assert!(err_output.to_string().contains(err_msg))
    }
}
