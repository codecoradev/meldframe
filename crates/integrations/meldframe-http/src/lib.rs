//! A thin axum adapter over the admission gate: REST (`POST /v1/files/{file}/tools/{tool}`) and WebSocket
//! (`GET /v1/ws`) both end in one handler, `Shared::submit`, which calls [`Submitter::submit`] and nothing
//! else. The adapter holds no policy: it authenticates, rate-limits, bounds size and time, parses, and maps
//! the answer to a stable code and an HTTP status. MCP is a third transport (E1.5) and is not here.
//!
//! Order of work for every call: client rate limit, then the token, then the principal's rate limit, and only
//! then is the body read and parsed. The submitter is synchronous (the server blocks on its store), so each
//! call runs on `spawn_blocking` under a timeout.

mod auth;
mod conn;
mod limit;
#[cfg(feature = "mcp")]
pub mod mcp;
mod reply;
mod rest;
mod ws;

pub use auth::{
    Authenticator, DevTokens, DevTokensRefused, MIN_TOKEN_LEN, SpecError, check_dev_listen,
    check_dev_tokens_allowed, dev_tokens_warning,
};
pub use conn::CappedListener;
pub use limit::{Clock, Rate, RateLimiter};
pub use meldframe_batch::Guard;
pub use reply::Failure;

use axum::{
    Router,
    routing::{get, post},
};
use meldframe_host::{Accepted, Op, Principal, Refusal};
use meldframe_protocol::Command;
use reply::Reply;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// The one way in to a write. The server implements it over `Server::submit_tool` (gate, then room).
/// Implementations block, so the adapter calls them from `spawn_blocking`.
pub trait Submitter: Send + Sync {
    fn submit(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure>;

    /// Like [`Submitter::submit`], but all or nothing as far as the checks of the gate and the room can tell
    /// beforehand: if any op would be refused, none is written and the failure names the op
    /// ([`Failure::Batch`]). MCP's `apply_commands` calls this unless the caller asks for `atomic: false`.
    fn submit_atomic(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
        guards: Vec<Guard>,
    ) -> Result<Vec<Accepted>, Failure>;
}

/// Limits of the adapter. A request over any of them is refused with its own code.
#[derive(Clone)]
pub struct Config {
    /// Most bytes of a request body or a WebSocket frame.
    pub max_body: usize,
    /// Longest the caller waits for a call: from the start of the blocking job, so it includes any wait for
    /// the server's lock. After that the caller gets `timeout`; the job is not cancelled and may still write.
    pub timeout: Duration,
    /// Longest the body of a request may take to arrive.
    pub body_timeout: Duration,
    /// Blocking calls running or abandoned at once, in all; one more is refused with `unavailable`. A call that
    /// timed out keeps its slot until its job really ends.
    pub max_in_flight: usize,
    /// The same, per principal.
    pub max_per_principal: usize,
    /// A WebSocket with no frame for this long is closed.
    pub ws_idle: Duration,
    /// A WebSocket is closed when its count of limited frames (up one per limited frame or ping, down one per
    /// allowed frame) reaches this.
    pub ws_max_limited: u32,
    /// Longest one reply or the closing frame may take to send; a client that does not read is dropped.
    pub ws_send_timeout: Duration,
    /// Budget per peer address, spent before the token is checked.
    pub per_client: Rate,
    /// Budget per principal (workspace and actor), spent before the body is read.
    pub per_principal: Rate,
    pub clock: Clock,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("max_body", &self.max_body)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_body: 1 << 20,
            timeout: Duration::from_secs(10),
            body_timeout: Duration::from_secs(10),
            max_in_flight: 32,
            max_per_principal: 4,
            ws_idle: Duration::from_secs(120),
            ws_max_limited: 5,
            ws_send_timeout: Duration::from_secs(10),
            per_client: Rate {
                burst: 60,
                per_second: 30,
            },
            per_principal: Rate {
                burst: 120,
                per_second: 60,
            },
            clock: {
                // Monotonic: a wall-clock step must not refill or freeze a bucket.
                let start = Instant::now();
                Arc::new(move || u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX))
            },
        }
    }
}

/// An op as a client sends it: the idempotency key, the command, the optional precondition and dependencies.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WireOp {
    key: String,
    command: Command,
    #[serde(default)]
    if_seq: Option<u64>,
    #[serde(default)]
    deps: Vec<String>,
}

impl From<WireOp> for Op {
    fn from(w: WireOp) -> Op {
        Op {
            key: w.key,
            command: w.command,
            if_seq: w.if_seq,
            deps: w.deps,
        }
    }
}

/// The key of a client address: an IPv4 address, or the /64 of an IPv6 one (a host can use any address of its
/// prefix); an IPv4-mapped IPv6 address is its IPv4. The rate limiter and the connection cap use it.
pub fn client_key(ip: std::net::IpAddr) -> String {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
    }
}

/// A place among the blocking calls; held by the job itself, so it is freed when the job ends, not when the
/// caller stops waiting.
struct Slot {
    _global: OwnedSemaphorePermit,
    per_principal: Arc<Mutex<HashMap<String, usize>>>,
    key: String,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut m = self.per_principal.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(n) = m.get_mut(&self.key) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                m.remove(&self.key);
            }
        }
    }
}

/// Characters that change how a name reads without showing: bidi controls, zero-width marks, line and
/// paragraph separators, the byte order mark.
fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2028}' | '\u{2029}'
        | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// A file name the server should see: 1 to 128 bytes, not `.` or `..`, not only whitespace, no slash or
/// backslash, no control or invisible formatting character.
pub(crate) fn valid_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && !name.trim().is_empty()
        && !name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control() || is_invisible(c))
}

/// A tool name: 1 to 64 bytes of ASCII letters, digits, `_`, `-` and `.`.
fn valid_tool(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

/// State of one router: both transports share it, so there is one path to the submitter.
pub(crate) struct Shared {
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    pub(crate) max_body: usize,
    pub(crate) body_timeout: Duration,
    pub(crate) ws_idle: Duration,
    pub(crate) ws_max_limited: u32,
    pub(crate) ws_send_timeout: Duration,
    timeout: Duration,
    in_flight: Arc<Semaphore>,
    per_principal: Arc<Mutex<HashMap<String, usize>>>,
    max_per_principal: usize,
    pub(crate) clients: RateLimiter,
    principals: RateLimiter,
    /// The per-principal budget as configured, for `whoami`.
    #[cfg_attr(not(feature = "mcp"), allow(dead_code))]
    pub(crate) principal_rate: Rate,
}

impl Shared {
    pub(crate) fn client_ok(&self, ip: &str) -> Result<(), Reply> {
        self.clients.check(ip).map_err(Reply::rate_limited)
    }

    /// The principal behind the token, without spending its budget (a connection is charged per frame).
    pub(crate) fn authenticate(&self, token: Option<&str>) -> Result<Principal, Reply> {
        token
            .and_then(|t| self.auth.authenticate(t))
            .ok_or_else(Reply::unauthenticated)
    }

    /// The principal behind the token, with one call charged to its budget.
    pub(crate) fn principal(&self, token: Option<&str>) -> Result<Principal, Reply> {
        let who = self.authenticate(token)?;
        self.principal_ok(&who)?;
        Ok(who)
    }

    /// Spend one call of the principal's budget.
    pub(crate) fn principal_ok(&self, who: &Principal) -> Result<(), Reply> {
        self.principals
            .check(&format!("{}\u{0}{}", who.workspace, who.actor))
            .map_err(Reply::rate_limited)
    }

    fn slot(&self, who: &Principal) -> Option<Slot> {
        let global = self.in_flight.clone().try_acquire_owned().ok()?;
        let key = format!("{}\u{0}{}", who.workspace, who.actor);
        let mut m = self.per_principal.lock().unwrap_or_else(|p| p.into_inner());
        let n = m.entry(key.clone()).or_insert(0);
        if *n >= self.max_per_principal {
            return None;
        }
        *n += 1;
        Some(Slot {
            _global: global,
            per_principal: self.per_principal.clone(),
            key,
        })
    }

    /// Run `job` on the blocking pool under the call's slot and the time limit: every call of every transport
    /// (a write, a read) goes through here, so one set of limits covers them all.
    pub(crate) async fn blocking<T: Send + 'static>(
        &self,
        who: &Principal,
        job: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, Reply> {
        let Some(slot) = self.slot(who) else {
            return Err(Reply::failure(&Failure::Unavailable(
                "too many calls in flight".into(),
            )));
        };
        let job = tokio::task::spawn_blocking(move || {
            let _slot = slot;
            job()
        });
        match tokio::time::timeout(self.timeout, job).await {
            Err(_) => Err(Reply::timeout()),
            Ok(Err(_)) => Err(Reply::internal()),
            Ok(Ok(done)) => Ok(done),
        }
    }

    /// The shared handler of REST and WebSocket: each op is its own write.
    pub(crate) async fn submit(
        &self,
        who: Principal,
        tool: String,
        file: String,
        ops: Vec<WireOp>,
    ) -> Reply {
        let ops = ops.into_iter().map(Op::from).collect();
        self.submit_ops(who, tool, file, ops, None).await
    }

    /// The one handler every transport ends in, MCP included: validate the names, take a slot, call the
    /// submitter (`Some(guards)` picks [`Submitter::submit_atomic`]) and map the answer.
    pub(crate) async fn submit_ops(
        &self,
        who: Principal,
        tool: String,
        file: String,
        ops: Vec<Op>,
        atomic: Option<Vec<Guard>>,
    ) -> Reply {
        if !valid_file(&file) || !valid_tool(&tool) {
            return Reply::failure(&Failure::Refused(Refusal::Invalid {
                reason: BAD_NAME.into(),
            }));
        }
        let submitter = self.submitter.clone();
        let sent = who.clone();
        let done = self
            .blocking(&who, move || {
                if let Some(guards) = atomic {
                    submitter.submit_atomic(&tool, &file, &sent, ops, guards)
                } else {
                    submitter.submit(&tool, &file, &sent, ops)
                }
            })
            .await;
        match done {
            Err(reply) => reply,
            Ok(Ok(accepted)) => Reply::accepted(&accepted),
            Ok(Err(failure)) => {
                if let Failure::Unavailable(why) | Failure::Permanent(why) = &failure {
                    eprintln!("meldframe-http: unavailable: {why:?}");
                }
                Reply::failure(&failure)
            }
        }
    }
}

pub(crate) const BAD_NAME: &str = "a file name is 1 to 128 bytes without slashes or control characters, and a tool name is 1 to 64 letters, digits, _ - or .";

fn shared(
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    config: Config,
) -> Arc<Shared> {
    Arc::new(Shared {
        auth,
        submitter,
        max_body: config.max_body,
        body_timeout: config.body_timeout,
        ws_idle: config.ws_idle,
        ws_max_limited: config.ws_max_limited,
        ws_send_timeout: config.ws_send_timeout,
        timeout: config.timeout,
        in_flight: Arc::new(Semaphore::new(config.max_in_flight.max(1))),
        per_principal: Arc::default(),
        max_per_principal: config.max_per_principal.max(1),
        clients: RateLimiter::new(config.per_client, config.clock.clone()),
        principals: RateLimiter::new(config.per_principal, config.clock),
        principal_rate: config.per_principal,
    })
}

fn routes(shared: Arc<Shared>) -> Router {
    Router::new()
        .route("/v1/files/{file}/tools/{tool}", post(rest::submit))
        .route("/v1/ws", get(ws::upgrade))
        .with_state(shared)
}

/// The router: REST and WebSocket over one submitter, one authenticator and one set of limits.
pub fn router(
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    config: Config,
) -> Router {
    routes(shared(auth, submitter, config))
}

/// The router with MCP (`/mcp`, streamable HTTP) added: the three transports share one authenticator, one set
/// of limits and one submitter, and MCP's reads go through `reader` on the same blocking pool.
#[cfg(feature = "mcp")]
pub fn router_with_mcp(
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    reader: Arc<dyn mcp::Reader>,
    config: Config,
) -> Router {
    router_with_mcp_config(auth, submitter, reader, config, mcp::McpConfig::default())
}

/// [`router_with_mcp`] with the MCP settings given.
#[cfg(feature = "mcp")]
pub fn router_with_mcp_config(
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    reader: Arc<dyn mcp::Reader>,
    config: Config,
    mcp: mcp::McpConfig,
) -> Router {
    let shared = shared(auth, submitter, config);
    routes(shared.clone()).merge(mcp::routes(shared, reader, mcp))
}
