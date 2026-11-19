//! A thin axum adapter over the admission gate: REST (`POST /v1/files/{file}/tools/{tool}`) and WebSocket
//! (`GET /v1/ws`) both end in one handler, `Shared::submit`, which calls [`Submitter::submit`] and nothing
//! else. The adapter holds no policy: it authenticates, rate-limits, bounds size and time, parses, and maps
//! the answer to a stable code and an HTTP status.
//!
//! Order of work for every call: client rate limit, the token, the principal's rate limit, and only then the
//! body. The submitter blocks on its store, so each call runs on `spawn_blocking` under a timeout.

mod auth;
mod conn;
#[cfg(feature = "mcp")]
pub mod mcp;
mod reply;
mod rest;
mod slots;
mod ws;

pub use auth::{
    Authenticator, DevTokens, DevTokensRefused, MIN_TOKEN_LEN, SpecError, check_dev_listen,
    check_dev_tokens_allowed, dev_tokens_warning,
};
pub use conn::{CappedListener, real_client, serve};
pub use meldframe_batch::Guard;
pub use meldframe_ratelimit::{Clock, Rate, RateLimiter};
pub use reply::{Failure, Reply};
pub use rest::{bearer, peer};
pub use slots::{Slot, Slots};

use axum::{
    Router,
    routing::{get, post},
};
use meldframe_host::{Accepted, Op, Principal, Refusal};
use meldframe_protocol::Command;
use serde::Deserialize;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// The one way in to a write (the server's `Server::submit_tool`); it blocks, so it runs on `spawn_blocking`.
pub trait Submitter: Send + Sync {
    fn submit(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure>;

    /// Like [`Submitter::submit`], but all or nothing as far as the gate and the room can tell beforehand: if any op
    /// would be refused, none is written and the failure names it ([`Failure::Batch`]). MCP's `apply_commands` uses it.
    fn submit_atomic(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
        guards: Vec<Guard>,
    ) -> Result<Vec<Accepted>, Failure>;
}

/// Limits of the adapter; a request over one is refused with its own code.
#[derive(Clone)]
pub struct Config {
    /// Most bytes of a request body or a WebSocket frame.
    pub max_body: usize,
    /// Longest the caller waits for a call (the server's lock included); then `timeout`, and the job may still write.
    pub timeout: Duration,
    /// Longest the body of a request may take to arrive.
    pub body_timeout: Duration,
    /// Blocking calls running or abandoned at once, in all; one more is `unavailable`. A timed out call keeps its slot.
    pub max_in_flight: usize,
    /// The same, per principal.
    pub max_per_principal: usize,
    /// A WebSocket with no frame for this long is closed.
    pub ws_idle: Duration,
    /// A WebSocket is closed when its count of limited frames (up one per limited, down one per allowed) reaches this.
    pub ws_max_limited: u32,
    /// Longest a reply or the closing frame may take to send; a client that does not read is dropped.
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

/// A file name the server should see: 1 to 128 bytes, not `.` or `..`, not only whitespace, no slash or
/// backslash, no control or invisible formatting character.
pub fn valid_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && !name.trim().is_empty()
        && !name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control() || meldframe_mcp_defs::is_invisible(c))
}

/// A tool name: 1 to 64 bytes of ASCII letters, digits, `_`, `-` and `.`.
fn valid_tool(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

/// Called with the principal and the file after a call wrote at least one op (the live transport wakes its
/// sockets with it).
pub type WriteHook = Box<dyn Fn(&Principal, &str) + Send + Sync>;

/// State of one router: every transport shares it, so there is one path to the submitter. Another crate (the
/// live WebSocket) builds on its checks and limits: `client_ok`, `principal`, `principal_ok`, `blocking`.
pub struct Shared {
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    pub max_body: usize,
    pub body_timeout: Duration,
    pub(crate) ws_idle: Duration,
    pub ws_max_limited: u32,
    pub ws_send_timeout: Duration,
    pub timeout: Duration,
    slots: Slots,
    /// Set once by a transport that wants to know about writes.
    pub on_write: OnceLock<WriteHook>,
    pub(crate) clients: RateLimiter,
    principals: RateLimiter,
    /// The per-principal budget as configured, for `whoami`.
    #[cfg_attr(not(feature = "mcp"), allow(dead_code))]
    pub(crate) principal_rate: Rate,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared").finish_non_exhaustive()
    }
}

impl Shared {
    pub fn client_ok(&self, ip: &str) -> Result<(), Reply> {
        self.clients.check(ip).map_err(Reply::rate_limited)
    }

    /// The principal behind the token, without spending its budget (a connection is charged per frame).
    pub(crate) fn authenticate(&self, token: Option<&str>) -> Result<Principal, Reply> {
        token
            .and_then(|t| self.auth.authenticate(t))
            .ok_or_else(Reply::unauthenticated)
    }

    /// The principal behind the token, with one call charged to its budget.
    pub fn principal(&self, token: Option<&str>) -> Result<Principal, Reply> {
        let who = self.authenticate(token)?;
        self.principal_ok(&who)?;
        Ok(who)
    }

    /// Spend one call of the principal's budget.
    pub fn principal_ok(&self, who: &Principal) -> Result<(), Reply> {
        self.principal_wait(who, 1).map_err(Reply::rate_limited)
    }

    /// Spend `n` calls of the principal's budget; `Err` says how long until a call is available. Calls spent
    /// before one was refused stay spent.
    pub fn principal_wait(&self, who: &Principal, n: u32) -> Result<(), Duration> {
        let key = format!("{}\u{0}{}", who.workspace, who.actor);
        (0..n).try_for_each(|_| self.principals.check(&key))
    }

    /// Run `job` on the blocking pool under the call's slot and the time limit: every call of every transport
    /// (a write, a read) goes through here, so one set of limits covers them all.
    pub async fn blocking<T: Send + 'static>(
        &self,
        who: &Principal,
        job: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, Reply> {
        let Some(slot) = self.slots.take(who) else {
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
        let (sent, woken) = (who.clone(), file.clone());
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
            Ok(Ok(accepted)) => {
                if let (Some(hook), true) =
                    (self.on_write.get(), accepted.iter().any(|a| a.written))
                {
                    hook(&who, &woken);
                }
                Reply::accepted(&accepted)
            }
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

/// The state of a router: build it once, then give it to [`routes`] and to any other transport's routes.
pub fn shared(
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
        slots: Slots::new(config.max_in_flight, config.max_per_principal),
        on_write: OnceLock::new(),
        clients: RateLimiter::new(config.per_client, config.clock.clone()),
        principals: RateLimiter::new(config.per_principal, config.clock),
        principal_rate: config.per_principal,
    })
}

/// REST and `/v1/ws` over `shared`.
pub fn routes(shared: Arc<Shared>) -> Router {
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

/// The router with MCP (`/mcp`) added: all three transports share one authenticator, limits and submitter.
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
