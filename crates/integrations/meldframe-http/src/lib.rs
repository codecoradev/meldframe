//! A thin axum adapter over the admission gate: REST (`POST /v1/files/{file}/tools/{tool}`) and WebSocket
//! both end in one handler, `Shared::submit`, which calls [`Submitter::submit`] and nothing
//! else. The adapter holds no policy: it authenticates, rate-limits, bounds size and time, parses, and maps
//! the answer to a stable code and an HTTP status. MCP is a third transport (E1.5) and is not here.
//!
//! Order of work for every call: client rate limit, then the token, then the principal's rate limit, and only
//! then is the body read and parsed. The submitter is synchronous (the server blocks on its store), so each
//! call runs on `spawn_blocking` under a timeout.

mod auth;
mod limit;
mod reply;
mod rest;

pub use auth::{Authenticator, DevTokens, DevTokensRefused, SpecError, check_dev_tokens_allowed};
pub use limit::{Clock, Rate, RateLimiter};
pub use reply::Failure;

use axum::{Router, routing::post};
use meldframe_host::{Accepted, Op, Principal};
use meldframe_protocol::Command;
use reply::Reply;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
}

/// Limits of the adapter. A request over any of them is refused with its own code.
#[derive(Clone)]
pub struct Config {
    /// Most bytes of a request body or a WebSocket frame.
    pub max_body: usize,
    /// Longest a call may take; after that the caller gets `timeout` (the write may still finish).
    pub timeout: Duration,
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
            per_client: Rate {
                burst: 60,
                per_second: 30,
            },
            per_principal: Rate {
                burst: 120,
                per_second: 60,
            },
            clock: Arc::new(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
            }),
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

/// State of one router: both transports share it, so there is one path to the submitter.
pub(crate) struct Shared {
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    pub(crate) max_body: usize,
    timeout: Duration,
    pub(crate) clients: RateLimiter,
    principals: RateLimiter,
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

    /// The shared handler: both transports call this, and it calls only `Submitter::submit`.
    pub(crate) async fn submit(
        &self,
        who: Principal,
        tool: String,
        file: String,
        ops: Vec<WireOp>,
    ) -> Reply {
        let submitter = self.submitter.clone();
        let ops: Vec<Op> = ops.into_iter().map(Op::from).collect();
        let job = tokio::task::spawn_blocking(move || submitter.submit(&tool, &file, &who, ops));
        match tokio::time::timeout(self.timeout, job).await {
            Err(_) => Reply::timeout(),
            Ok(Err(_)) => Reply::internal(),
            Ok(Ok(Ok(accepted))) => Reply::accepted(&accepted),
            Ok(Ok(Err(failure))) => Reply::failure(&failure),
        }
    }
}

/// The router: REST and WebSocket over one submitter, one authenticator and one set of limits.
pub fn router(
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    config: Config,
) -> Router {
    let shared = Arc::new(Shared {
        auth,
        submitter,
        max_body: config.max_body,
        timeout: config.timeout,
        clients: RateLimiter::new(config.per_client, config.clock.clone()),
        principals: RateLimiter::new(config.per_principal, config.clock),
    });
    Router::new()
        .route("/v1/files/{file}/tools/{tool}", post(rest::submit))
        .with_state(shared)
}
