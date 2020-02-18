use std::str::FromStr as _;

use anyhow::{anyhow, Result};
use http::header::{HeaderMap, HeaderName, HeaderValue};
use oasis_types::{Address, RpcError};
use uuid::Uuid;

#[cfg(not(target_env = "sgx"))]
use reqwest::Client;

use crate::api::*;

pub trait Gateway {
    /// Deploys a new service with the provided initcode.
    /// `initcode` is expected to be the Wasm bytecode concatenated with the the constructor stdin.
    /// Upon success, returns the address of the new service.
    fn deploy(&self, initcode: &[u8]) -> Result<Address, RpcError>;

    /// Returns the output of calling the service at `address` with `data` as stdin.
    fn rpc(&self, address: Address, payload: &[u8]) -> Result<Vec<u8>, RpcError>;
}

/// Holds necessary information to make http requests to the gateway.
///
/// # Example
///
/// ```no_run
/// use oasis_client::Gateway as _;
///
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
    headers: HeaderMap,

    /// The http session used for sending http requests.
    client: Client,

    /// A polling service used to receive responses from asynchronous requests made.
    polling_params: PollingParams,
}

#[derive(Clone, Debug)]
pub struct HttpGatewayBuilder {
    url: String,
    api_key: Option<String>,
    headers: HeaderMap,
    polling_params: PollingParams,
}

impl HttpGatewayBuilder {
    pub fn new(url: impl AsRef<str>) -> Self {
        Self {
            url: url.as_ref().to_string(),
            ..Default::default()
        }
    }

    /// Set the api key expected by the Oasis Developer gateway.
    pub fn api_key(mut self, api_key: impl AsRef<str>) -> Self {
        self.api_key = Some(api_key.as_ref().to_string());
        self
    }

    /// Append the value of the named header.
    pub fn header(mut self, name: impl AsRef<[u8]>, value: impl AsRef<[u8]>) -> Result<Self> {
        self.headers.insert(
            HeaderName::from_bytes(name.as_ref())?,
            HeaderValue::from_bytes(value.as_ref())?,
        );
        Ok(self)
    }

    /// Assign the provided headers as the defaults for all requests made by the `HttpGateway`.
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Set the polling parameters for the `HttpGateway`.
    pub fn polling_params(mut self, params: PollingParams) -> Self {
        self.polling_params = params;
        self
    }

    /// Creates a new `HttpGateway` instance that is configured with headers expected by the
    /// Oasis Developer gateway.
    pub fn build(self) -> HttpGateway {
        let session_key = Uuid::new_v4().to_string();

        let mut headers = self.headers;
        headers.insert("X-OASIS-INSECURE-AUTH", HeaderValue::from_static("1"));
        if let Some(api_key) = self.api_key {
            headers.insert(
                "X-OASIS-LOGIN-TOKEN",
                HeaderValue::from_str(&api_key).unwrap(),
            );
        }
        headers.insert(
            "X-OASIS-SESSION-KEY",
            HeaderValue::from_str(&session_key).unwrap(),
        );

        HttpGateway::new(self.url, headers, self.polling_params)
    }
}

impl Default for HttpGatewayBuilder {
    fn default() -> Self {
        Self {
            url: "https://gateway.devnet.oasiscloud.io".to_string(),
            api_key: None,
            headers: HeaderMap::new(),
            polling_params: PollingParams::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PollingParams {
    /// Interval between sending requests in milliseconds.
    pub sleep_duration: u64,

    /// Number of attempts before giving up with an error.
    pub max_attempts: u32,
}

impl Default for PollingParams {
    fn default() -> Self {
        Self {
            sleep_duration: 500,
            max_attempts: 20,
        }
    }
}

impl HttpGateway {
    /// Creates a new `HttpGateway` pointed at `url` and with default `headers`.
    pub fn new(url: String, headers: HeaderMap, polling_params: PollingParams) -> Self {
        Self {
            url,
            headers,
            client: Client::new(),
            polling_params,
        }
    }

    /// Submit given request asynchronously and poll for results.
    fn post_and_poll(&self, api: DeveloperGatewayApi, body: GatewayRequest) -> Result<Event> {
        let response: AsyncResponse = self.request(api.method, api.url, body)?;

        match self.poll_for_response(response.id)? {
            Event::Error { description, .. } => Err(anyhow!("{}", description)),
            e => Ok(e),
        }
    }

    /// Synchronous polling. Repeatedly attempts to retrieve the event of the given
    /// request id. If polling fails `max_attempts` times an error is returned.
    fn poll_for_response(&self, request_id: u64) -> Result<Event> {
        let PollingParams {
            sleep_duration,
            max_attempts,
        } = self.polling_params;

        let poll_request = GatewayRequest::Poll {
            offset: request_id,
            count: 1, // poll for a single event
            discard_previous: true,
        };

        for attempt in 0..max_attempts {
            let events: PollEventResponse = self.request(
                SERVICE_POLL_API.method,
                &SERVICE_POLL_API.url,
                &poll_request,
            )?;

            // we polled for a singe event so we want the first event in the list, if it exists.
            let event = events.events.first();
            if let Some(e) = event {
                return Ok(e.clone());
            }

            info!(
                "polling... (request id: {}, attempt: {})",
                request_id, attempt
            );

            #[cfg(not(target_env = "sgx"))]
            std::thread::sleep(std::time::Duration::from_millis(sleep_duration));

            // `sleep` is not supported in EDP. Spin wait instead.
            #[cfg(target_env = "sgx")]
            {
                let start = std::time::Instant::now();
                let duration = std::time::Duration::from_millis(sleep_duration);
                while start.elapsed() < duration {
                    std::thread::yield_now();
                }
            }
        }
        Err(anyhow!("Exceeded max polling attempts"))
    }

    /// Submits a request to the gateway. The body of the request is json-serialized and the
    /// response is expected to be json-serialized as well.
    fn request<P: serde::Serialize, Q: serde::de::DeserializeOwned>(
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

        let mut res = builder
            .headers(self.headers.clone())
            .json(&payload)
            .send()?;
        if res.status().is_success() {
            Ok(res.json()?)
        } else {
            Err(anyhow!("gateway returned error: {}", res.status()))
        }
    }
}

impl Gateway for HttpGateway {
    fn deploy(&self, initcode: &[u8]) -> std::result::Result<Address, RpcError> {
        let initcode_hex = hex::encode(initcode);
        info!("deploying service `{}`", &initcode_hex[..32]);

        let body = GatewayRequest::Deploy {
            data: format!("0x{}", initcode_hex),
        };

        self.post_and_poll(SERVICE_DEPLOY_API, body)
            .and_then(|event| {
                match event {
                    Event::DeployService { address, .. } => {
                        Ok(Address::from_str(&address[2..] /* strip 0x */)?)
                    }
                    e => Err(anyhow!("expecting `DeployService` event. got {:?}", e)),
                }
            })
            .map_err(RpcError::GatewayError)
    }

    fn rpc(&self, address: Address, payload: &[u8]) -> std::result::Result<Vec<u8>, RpcError> {
        info!("making RPC to {}", address);

        let body = GatewayRequest::Execute {
            address: address.to_string(),
            data: format!("0x{}", hex::encode(payload)),
        };

        self.post_and_poll(SERVICE_EXECUTE_API, body)
            .and_then(|event| match event {
                Event::ExecuteService { output, .. } => Ok(dbg!(hex::decode(&output[2..])?)),
                e => Err(anyhow!("expecting `ExecuteService` event. got {:?}", e)),
            })
            .map_err(RpcError::GatewayError)
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
