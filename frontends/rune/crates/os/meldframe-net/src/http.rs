//! One HTTP client for every REST call of the shell: the bearer token on each request, a bound on time and on the size of an
//! answer, no proxy (a token must not be handed to whatever `HTTP_PROXY` names) and every status an answer, not an error: the
//! app layer reads a refusal, so the shell hands over the status and the text as they came.

use crate::config::Config;
use std::time::Duration;

/// How long one request may take, from the first byte sent to the last byte read.
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// The most text of an answer the shell reads: a snapshot of a large file is a few megabytes.
pub const MAX_ANSWER: u64 = 32 * 1024 * 1024;

/// The verbs of the routes the shell calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Put,
    Post,
}

/// A request that got no answer: the network failed, or the answer was not text within the bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoAnswer;

/// The status and the text of an answer, not read.
pub type Answer = (u16, String);

#[derive(Debug, Clone)]
pub struct Http {
    agent: ureq::Agent,
    config: Config,
}

impl Http {
    pub fn new(config: &Config) -> Http {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .proxy(None)
            .timeout_global(Some(TIMEOUT))
            .build()
            .into();
        Http {
            agent,
            config: config.clone(),
        }
    }

    /// One request. `Err` only when no answer came.
    pub fn call(&self, method: Method, path: &str, body: Option<&str>) -> Result<Answer, NoAnswer> {
        let url = self.config.server.http(path);
        let auth = self.config.token.bearer();
        let sent = match method {
            Method::Get => self.agent.get(&url).header("authorization", &auth).call(),
            Method::Put | Method::Post => {
                let builder = if method == Method::Put {
                    self.agent.put(&url)
                } else {
                    self.agent.post(&url)
                };
                builder
                    .header("authorization", &auth)
                    .header("content-type", "application/json")
                    .send(body.unwrap_or("{}"))
            }
        };
        let mut response = sent.map_err(|_| NoAnswer)?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .with_config()
            .limit(MAX_ANSWER)
            .lossy_utf8(true)
            .read_to_string()
            .map_err(|_| NoAnswer)?;
        Ok((status, text))
    }
}
