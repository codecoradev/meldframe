//! A thin axum adapter over the admission gate: REST (`POST /v1/files/{file}/tools/{tool}`) and WebSocket
//! (`GET /v1/ws`) both end in one handler, `Shared::submit`, which calls [`Submitter::submit`] and nothing
//! else. The adapter holds no policy: it authenticates, rate-limits, bounds size and time, parses, and maps
//! the answer to a stable code and an HTTP status.
//!
//! Order of work for every call: client rate limit, the token, the principal's rate limit, and only then the
//! body. The submitter blocks on its store, so each call runs on `spawn_blocking` under a timeout.

mod auth;
mod config;
mod conn;
#[cfg(feature = "mcp")]
pub mod mcp;
mod names;
mod reply;
mod rest;
mod slots;
mod submitter;
mod wire;
mod ws;

pub use auth::{
    Authenticator, DevTokens, DevTokensRefused, MIN_TOKEN_LEN, SpecError, check_dev_listen,
    check_dev_tokens_allowed, dev_tokens_warning,
};
pub use config::Config;
pub use conn::{CappedListener, real_client, serve};
pub use meldframe_batch::Guard;
pub use meldframe_ratelimit::{Clock, Rate, RateLimiter};
pub use names::{client_key, valid_file};
pub use reply::{Failure, Reply};
pub use rest::{bearer, peer};
pub use slots::{Slot, Slots};
pub use submitter::Submitter;

use axum::{
    Router,
    routing::{get, post},
};
use meldframe_activity::BatchSink;
use meldframe_checkpoints::{RunCall, RunSink, is_agent};
use meldframe_host::{Op, Principal, Refusal};
use names::{BAD_NAME, valid_tool};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use wire::WireOp;

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
    /// Set once by the server that keeps batch headers; without it `apply_commands` refuses a `note`.
    pub batches: OnceLock<Arc<dyn BatchSink>>,
    /// Set once by the server that keeps checkpoints: told after an agent's write, so a run start takes a row.
    pub runs: OnceLock<Arc<dyn RunSink>>,
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
    pub async fn submit_ops(
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
        // An agent's call keeps its keys, so a call that fails after it wrote a prefix is found by key; any other
        // call costs the prefix test.
        let run = self
            .runs
            .get()
            .filter(|_| is_agent(&who.actor))
            .map(|sink| {
                let keys = ops.iter().map(|o| o.key.clone()).collect();
                (
                    sink.clone(),
                    RunCall {
                        file: file.clone(),
                        keys,
                    },
                )
            });
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
                let seen: Vec<(u64, bool)> = accepted.iter().map(|a| (a.seq, a.written)).collect();
                self.run_started(&who, run, Some(seen)).await;
                Reply::accepted(&accepted)
            }
            Ok(Err(failure)) => {
                self.run_started(&who, run, None).await;
                if let Failure::Unavailable(why) | Failure::Permanent(why) = &failure {
                    eprintln!("meldframe-http: unavailable: {why:?}");
                }
                Reply::failure(&failure)
            }
        }
    }
}

impl Shared {
    /// Tell the run sink about an agent's call that ran. `done` is what a call that succeeded accepted (a run starts
    /// only when it wrote an op and the in-memory detector says so); `None` is a call that failed, which the sink
    /// looks into on its own. The store write is its own job under the caller's slot, so it never holds the call's own
    /// job, and nothing here changes the reply; the reply does wait for the job, for at most the call's time limit, as
    /// it does for the batch sink. A job that cannot run, outlasts the limit or panics is reported to the sink, which
    /// logs and counts it and gives the claim back.
    async fn run_started(
        &self,
        who: &Principal,
        run: Option<(Arc<dyn RunSink>, RunCall)>,
        done: Option<Vec<(u64, bool)>>,
    ) {
        let Some((sink, call)) = run else {
            return;
        };
        if let Some(done) = &done
            && !(done.iter().any(|d| d.1) && sink.starts(&who.workspace, &call.file, &who.actor))
        {
            return;
        }
        let (w, file, taker) = (who.clone(), call.file.clone(), sink.clone());
        let job = move || taker.take(&w.workspace, &w.actor, &call, done.as_deref());
        if let Err(reply) = self.blocking(who, job).await {
            let why = reply.body["error"]["code"].as_str().unwrap_or("internal");
            sink.lost(&who.workspace, &file, &who.actor, why);
        }
    }
}

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
        batches: OnceLock::new(),
        runs: OnceLock::new(),
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
