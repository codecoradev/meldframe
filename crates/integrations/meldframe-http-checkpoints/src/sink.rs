//! The run sink of the server: the detector, the store the rows go to and the reader that finds what a failed call wrote.

use crate::gate::{Clock, Publish, StateLimits};
use crate::wire;
use checkpoint_store::CheckpointStore;
use meldframe_checkpoints::{
    NewCheckpoint, RunCall, RunSink, RunWatch, is_agent, seq_before_first,
};
use meldframe_host::Principal;
use meldframe_http::Shared;
use meldframe_http::mcp::Reader;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub(crate) live: Option<Publish>,
    limits: StateLimits,
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
            limits: StateLimits::default(),
        }
    }

    /// The same, with bounds of the state route of its own (a test's).
    #[must_use]
    pub fn with_state_limits(mut self, limits: StateLimits) -> Self {
        self.limits = limits;
        self
    }

    pub(crate) fn limits(&self) -> StateLimits {
        self.limits
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
