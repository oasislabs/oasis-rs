use std::{thread, time};

use anyhow::{anyhow, Result};

use crate::{api, HttpGateway};

#[cfg(target_env = "sgx")]
use std::time::Instant;

#[derive(Debug)]
pub struct PollingService {
    /// Interval between sending requests.
    sleep_duration: u64,
    /// Number of attempts before giving up with an error.
    max_attempts: u32,
}

impl PollingService {
    pub fn new(sleep_duration: u64, max_attempts: u32) -> Self {
        Self {
            sleep_duration,
            max_attempts,
        }
    }

    /// Synchronous polling. Repeatedly attempts to retrieve the event of the given
    /// request id. If polling fails `max_attempts` times an error is returned.
    pub fn poll_for(&self, gateway: &HttpGateway, request_id: u64) -> Result<api::Event> {
        let api = api::SERVICE_POLL_API;

        let mut attempts = 0;
        loop {
            let body = api::GatewayRequest::Poll {
                offset: request_id,
                count: 1, // poll for a single event
                discard_previous: true,
            };

            let events: api::PollEventResponse = gateway.request(api.method, &api.url, body)?;

            // we polled for a singe event so we want the first event in the list, if it exists.
            let event = events.events.first();
            if let Some(e) = event {
                return Ok(e.clone());
            }

            info!(
                "Polling... (request id: {}, attempt: {})",
                request_id, attempts
            );

            if attempts > self.max_attempts {
                error!("Exceeded max polling attempts");
                return Err(anyhow!("Exceeded max polling attempts"));
            }

            #[cfg(not(target_env = "sgx"))]
            thread::sleep(time::Duration::from_millis(self.sleep_duration));

            // `sleep` is not supported in EDP. Spin wait instead.
            #[cfg(target_env = "sgx")]
            {
                let start = Instant::now();
                let duration = time::Duration::from_millis(self.sleep_duration);
                while start.elapsed() < duration {
                    thread::yield_now();
                }
            }

            attempts += 1;
        }
    }
}
