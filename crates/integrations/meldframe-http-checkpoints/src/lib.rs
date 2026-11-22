//! The server side of checkpoints over HTTP (E5.3, ADR-0023). [`install`] is the one call the binary makes: it sets the
//! [`RunSink`] on `meldframe_http::Shared`, the place every write passes (REST, the submit socket and MCP end in
//! `Shared::submit_ops`), so an agent's run start takes a row with no editor open.
//!
//! The detector is [`RunWatch`] of `meldframe-checkpoints`: only an actor with the `agent:` prefix starts a run, the
//! first write after a gap of more than 5 minutes (or the first seen, also after a restart) takes a `runStart` row at the
//! `seq` just before the first op the call wrote. A call that failed after it wrote a prefix is found by key through
//! `Reader::written`, the unlogged read the batch sink uses too. The common path costs a prefix test; only a due run
//! start runs a job, its own job under the caller's slot, so it never holds the call's own job. The reply waits for it
//! for at most the call's time limit (as for the batch sink). A write the store refuses, a job that cannot run or one
//! that outlasts the limit is logged and counted ([`Checkpoints::failures`]), gives the claim back so the next call of
//! the run can take the run start, and never changes the reply; a job that outlasts the limit may still finish, which
//! costs one extra row.
//!
//! A row that was made, a run start or a person's, is queued as a `checkpoint` frame on the live streams of the file
//! (ephemeral, with headroom, through the publisher of [`Gate`]); the routes in `routes.rs` list the rows and let a
//! listed person make one. [`CheckpointsModule`] registers the two calls of a person, `checkpoint_create` and
//! `checkpoint_state`, so a deployment can deny them and the call log records their tier; neither is an MCP tool.
//! The route of `checkpoint_state` follows.

mod routes;
mod wire;

use checkpoint_store::CheckpointStore;
use meldframe_checkpoints::{
    NewCheckpoint, RunCall, RunSink, RunWatch, is_agent, seq_before_first,
};
use meldframe_host::Principal;
use meldframe_http::Shared;
use meldframe_http::mcp::Reader;
use meldframe_module::{Module, Policy, Registrar, RiskTier};
use meldframe_protocol::ServerMsg;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

/// The call that makes a checkpoint, a person's: a Change, not offered over MCP.
pub const CREATE_TOOL: &str = "checkpoint_create";
pub const CREATE_DESCRIPTION: &str = "Make a checkpoint of a file: a labelled point in its history that a person can restore. A person's decision, made through the route POST /v1/files/{file}/checkpoints; not offered over MCP.";
/// The call that reads a file as it was at a checkpoint, a person's: a Read, not offered over MCP.
pub const STATE_TOOL: &str = "checkpoint_state";
pub const STATE_DESCRIPTION: &str = "Read a file as it was at a checkpoint. A person's read, made through the route GET /v1/files/{file}/checkpoints/{n}/state; not offered over MCP.";

/// Registers the two calls in the registry. Without it the registry has neither.
#[derive(Debug, Clone, Copy)]
pub struct CheckpointsModule;

impl Module for CheckpointsModule {
    fn name(&self) -> &'static str {
        "checkpoints"
    }

    fn register(&self, r: &mut Registrar) {
        r.agent_tool(CREATE_TOOL, CREATE_DESCRIPTION, RiskTier::Change);
        r.agent_tool(STATE_TOOL, STATE_DESCRIPTION, RiskTier::Read);
    }
}

/// Queues a frame on the live streams of a file of the principal's workspace, as ephemeral and with headroom; answers
/// how many streams took it. The server's `SharedServer::publish_with_headroom` is the one.
pub type Publish = Arc<dyn Fn(&Principal, &str, ServerMsg) -> usize + Send + Sync>;

/// Whether an actor may make a checkpoint: until a principal has a kind, the listed reviewers of ADR-0021 (the binary
/// builds the closure from `MELDFRAME_REVIEWERS`, so this crate does not depend on the activity one).
pub type May = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// What the server hands the routes: who may make a row, what the registry's policy for [`CREATE_TOOL`] is (a row is
/// made only under `Auto`) and the publisher of the frame.
#[derive(Clone)]
pub struct Gate {
    pub may: May,
    pub policy: Option<Policy>,
    pub publish: Publish,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

/// The server clock in milliseconds.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

fn now_ms() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The run sink of the server: the detector, the store the rows go to and the reader that finds what a failed call wrote.
pub struct Checkpoints {
    store: Arc<dyn CheckpointStore>,
    reader: Arc<dyn Reader>,
    watch: Mutex<RunWatch>,
    clock: Clock,
    failures: AtomicU64,
    live: Option<Publish>,
}

impl std::fmt::Debug for Checkpoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checkpoints").finish_non_exhaustive()
    }
}

impl Checkpoints {
    pub fn new(store: Arc<dyn CheckpointStore>, reader: Arc<dyn Reader>) -> Self {
        Checkpoints {
            store,
            reader,
            watch: Mutex::new(RunWatch::new()),
            clock: Arc::new(now_ms),
            failures: AtomicU64::new(0),
            live: None,
        }
    }

    /// The same, with a clock of its own (a test's).
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// How many run starts were not taken because the store or the read failed (each was logged).
    pub fn failures(&self) -> u64 {
        self.failures.load(Ordering::SeqCst)
    }

    /// Give the claim of a run start back, so the next write of the actor in the file counts as one.
    fn forget(&self, workspace: &str, file: &str, actor: &str) {
        let mut watch = self.watch.lock().unwrap_or_else(PoisonError::into_inner);
        watch.forget(workspace, file, actor);
    }

    pub(crate) fn store(&self) -> &Arc<dyn CheckpointStore> {
        &self.store
    }

    pub(crate) fn now(&self) -> u64 {
        (self.clock)()
    }

    /// The head of a file, a read that is not a call of its own.
    pub(crate) fn head(&self, who: &Principal, file: &str) -> Result<u64, meldframe_http::Failure> {
        self.reader.head(who, file)
    }

    /// Queue the frame of a row that was made. A drop is silent: the list is the truth.
    pub(crate) fn announce(
        &self,
        who: &Principal,
        file: &str,
        row: &meldframe_checkpoints::Checkpoint,
    ) {
        if let Some(live) = &self.live {
            live(who, file, wire::frame(file, row));
        }
    }

    /// One entry of the call log, in a slot of the principal and under the time limit like every other call (a busy
    /// server loses the entry rather than holding the answer).
    pub(crate) async fn log(
        &self,
        shared: &Shared,
        who: &Principal,
        tool: &'static str,
        file: &str,
        code: Option<String>,
    ) {
        let (reader, w, file) = (self.reader.clone(), who.clone(), file.to_owned());
        let job = move || reader.record(&w, tool, &file, code.as_deref());
        let _ = shared.blocking(who, job).await;
    }

    fn failed(&self, what: &str, why: &dyn std::fmt::Display) {
        self.failures.fetch_add(1, Ordering::SeqCst);
        eprintln!("meldframe-http-checkpoints: {what}: {why}");
    }
}

impl RunSink for Checkpoints {
    fn starts(&self, workspace: &str, file: &str, actor: &str) -> bool {
        let now = (self.clock)();
        let mut watch = self.watch.lock().unwrap_or_else(PoisonError::into_inner);
        watch.starts(workspace, file, actor, now)
    }

    fn take(&self, workspace: &str, actor: &str, call: &RunCall, done: Option<&[(u64, bool)]>) {
        if !is_agent(actor) {
            return;
        }
        let seqs: Vec<u64> = match done {
            Some(done) => done.iter().filter(|d| d.1).map(|d| d.0).collect(),
            None => {
                let who = Principal::new(workspace, actor);
                match self.reader.written(&who, &call.file, &call.keys) {
                    Ok(found) => found,
                    Err(e) => {
                        return self
                            .failed("could not read what a failed call wrote", &format!("{e:?}"));
                    }
                }
            }
        };
        let Some(seq) = seq_before_first(seqs.iter().copied()) else {
            return;
        };
        if done.is_none() && !self.starts(workspace, &call.file, actor) {
            return;
        }
        let new = match NewCheckpoint::run_start(actor, seq) {
            Ok(new) => new,
            Err(e) => {
                self.failed("not a checkpoint", &e);
                return self.forget(workspace, &call.file, actor);
            }
        };
        let made = self
            .store
            .create(workspace, &call.file, None, &new, (self.clock)());
        match made {
            Ok(made) if made.created => {
                let who = Principal::new(workspace, actor);
                self.announce(&who, &call.file, &made.checkpoint);
            }
            Ok(_) => {}
            Err(e) => {
                self.failed("no run start kept", &e);
                self.forget(workspace, &call.file, actor);
            }
        }
    }

    fn lost(&self, workspace: &str, file: &str, actor: &str, why: &str) {
        self.failed("a run start was not taken", &why);
        self.forget(workspace, file, actor);
    }
}

/// Everything the server wires for checkpoints in one call: the sink is set on `shared`, so every write of an agent is
/// watched and a run start is announced, and the routes come back to merge into the router. A second install on the
/// same `Shared` keeps the first sink. The route of `checkpoint_state` joins here in the next slice, so the binary's
/// `main` does not grow with it.
pub fn install(
    shared: &Arc<Shared>,
    store: Arc<dyn CheckpointStore>,
    reader: Arc<dyn Reader>,
    gate: Gate,
) -> axum::Router {
    install_with(shared, Checkpoints::new(store, reader), gate).1
}

/// As [`install`], for a sink built by the caller (a test gives it a clock); the sink comes back too, for its failure
/// count.
pub fn install_with(
    shared: &Arc<Shared>,
    mut checkpoints: Checkpoints,
    gate: Gate,
) -> (Arc<Checkpoints>, axum::Router) {
    checkpoints.live = Some(gate.publish.clone());
    let checkpoints = Arc::new(checkpoints);
    let _ = shared.runs.set(checkpoints.clone());
    let router = routes::routes(shared.clone(), checkpoints.clone(), gate);
    (checkpoints, router)
}
