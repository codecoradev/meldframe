//! The state of one router: every transport shares it, so there is one path to the submitter and one set
//! of limits.

use crate::auth::Authenticator;
use crate::config::Config;
use crate::reply::{Failure, Reply};
use crate::slots::Slots;
use crate::submitter::Submitter;
use meldframe_activity::BatchSink;
use meldframe_checkpoints::RunSink;
use meldframe_host::Principal;
use meldframe_ratelimit::{Rate, RateLimiter};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Called with the principal and the file after a call wrote at least one op (the live transport wakes its
/// sockets with it).
pub type WriteHook = Box<dyn Fn(&Principal, &str) + Send + Sync>;

/// State of one router: every transport shares it, so there is one path to the submitter. Another crate (the
/// live WebSocket) builds on its checks and limits: `client_ok`, `principal`, `principal_ok`, `blocking`.
pub struct Shared {
    auth: Arc<dyn Authenticator>,
    pub(crate) submitter: Arc<dyn Submitter>,
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
}

/// The state of a router: build it once, then give it to [`crate::routes`] and to any other transport's routes.
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
