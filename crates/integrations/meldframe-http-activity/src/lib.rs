//! The activity panel's server side over HTTP (E5.1, ADR-0021). [`Activity`] is the one constructor: it holds the
//! batch store and the reader, and hands out the [`BatchSink`] that `apply_commands` calls after every call.
//!
//! A header is kept in three cases, all by [`BatchSink::record`]: a call that wrote (the normal case); a call that
//! failed after it wrote a prefix (the sink asks the reader how many of the call's keys are in the log, with
//! [`Reader::written`], which is not a `batch_status` read and writes no call-log entry); and a replay, the retry
//! after a timeout, which takes the header's `written` from the keys found and its time from the replay. A failed
//! header write never fails the call: it is logged and the answer says `batch: null`.
//!
//! The wire (slice 5): a header that was made or changed is queued as a `batch` frame on the live streams of the file,
//! after the call's op events and through the publisher given to [`Activity::with_live`] (the server sends it with
//! headroom: it never takes the last slots of a queue and so never causes a resync by itself, but it does use the
//! slack of a stalled reader); [`Activity::routes`] adds the list route, which is the
//! truth the frame is a hint of.
//!
//! The MCP tool `get_batches` (slice 6) is in `tool.rs`: [`tool`] gives it for `McpConfig::extra` and
//! [`ActivityModule`] registers it as a Read.

mod review;
mod reviewers;
mod route;
mod tool;
mod wire;

pub use review::{REVIEW_DESCRIPTION, REVIEW_TOOL};
pub use reviewers::{MAX_REVIEWERS, Reviewers, VAR as REVIEWERS_VAR};
pub use tool::{ActivityModule, DESCRIPTION, NOTICE, TOOL, input_schema, tool};

use activity_store::BatchStore;
use meldframe_activity::{BatchCall, BatchSink, Marked, NewBatch};
use meldframe_host::Principal;
use meldframe_http::Shared;
use meldframe_http::mcp::{Extra, Reader};
use meldframe_module::{Policy, Registry};
use meldframe_protocol::ServerMsg;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Queues a frame on the live streams of a file of the principal's workspace, as ephemeral and with headroom; answers
/// how many streams took it. The server's `SharedServer::publish_with_headroom` is the one.
pub type Publish = Arc<dyn Fn(&Principal, &str, ServerMsg) -> usize + Send + Sync>;

/// The activity panel's server side: the store of batch headers and the reader that counts a failed call's keys.
#[derive(Debug)]
pub struct Activity {
    sink: Arc<Sink>,
    setter: Arc<review::Setter>,
}

impl Activity {
    /// Headers are kept and listed, and no frame is sent.
    pub fn new(store: Arc<dyn BatchStore>, reader: Arc<dyn Reader>) -> Self {
        Self::with_live(store, reader, None)
    }

    /// As [`Activity::new`], and a header that changed is also sent through `live`.
    pub fn with_live(
        store: Arc<dyn BatchStore>,
        reader: Arc<dyn Reader>,
        live: Option<Publish>,
    ) -> Self {
        let setter = review::Setter::new(reader.clone(), live.clone());
        Activity {
            sink: Arc::new(Sink {
                store,
                reader,
                live,
            }),
            setter: Arc::new(setter),
        }
    }

    /// Names who may set a review state and what the registry's policy for [`REVIEW_TOOL`] is. Without it nobody
    /// may: the setter answers `forbidden` to every caller and `canReview` is false.
    #[must_use]
    pub fn with_review(mut self, reviewers: Reviewers, policy: Option<Policy>) -> Self {
        self.setter = Arc::new(self.setter.with(reviewers, policy));
        self
    }

    /// `GET /v1/files/{file}/batches` and `PUT /v1/files/{file}/batches/{n}/review`, on the limits and the tokens of
    /// `shared`.
    pub fn routes(&self, shared: Arc<Shared>) -> axum::Router {
        route::routes(shared, self.sink.store.clone(), self.setter.clone())
    }

    /// The sink to set on `meldframe_http::Shared::batches`.
    pub fn sink(&self) -> Arc<dyn BatchSink> {
        self.sink.clone()
    }
}

/// What the server reads before it hands the registry over: the policies for the two calls of this crate and the
/// list of reviewers.
#[derive(Debug, Clone)]
pub struct Policies {
    /// `get_batches`, a Read.
    pub get_batches: Option<Policy>,
    /// `batch_review`, a Change.
    pub review: Option<Policy>,
    /// Who may set a review state: [`REVIEWERS_VAR`].
    pub reviewers: Reviewers,
}

/// The policies of `registry` for [`TOOL`] and [`REVIEW_TOOL`], and the reviewers of the environment. A bad list is
/// an error; the binary reads it once before it opens the store, so a bad list fails before any side effect, and
/// again here to use it.
pub fn policies(registry: &Registry) -> Result<Policies, String> {
    Ok(Policies {
        get_batches: registry.policy_for(TOOL),
        review: registry.policy_for(REVIEW_TOOL),
        reviewers: Reviewers::from_env()?,
    })
}

/// Everything the server wires for the activity panel in one call: the sink is set on `shared` (so `apply_commands`
/// keeps headers, takes a `note` and sends the frame through `publish`) and the routes come back to merge into the
/// router. `tools` is the list the MCP route serves besides its own and the registry's policies: `get_batches` is
/// added to the list when its policy is `Auto`. The review setter is a route only, never a tool of that list, and
/// only the listed reviewers may use it. Later slices join here, so the binary's `main` does not grow with them.
pub fn install(
    shared: &Arc<Shared>,
    store: Arc<dyn BatchStore>,
    reader: Arc<dyn Reader>,
    publish: Publish,
    tools: (&mut Vec<Arc<dyn Extra>>, Policies),
) -> axum::Router {
    let (list, policy) = tools;
    list.extend(tool(shared.clone(), store.clone(), policy.get_batches));
    let activity = Activity::with_live(store, reader, Some(publish));
    let activity = activity.with_review(policy.reviewers, policy.review);
    // Set once: a second install on the same `Shared` keeps the first sink.
    let _ = shared.batches.set(activity.sink());
    activity.routes(shared.clone())
}

struct Sink {
    store: Arc<dyn BatchStore>,
    reader: Arc<dyn Reader>,
    live: Option<Publish>,
}

impl std::fmt::Debug for Sink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sink").finish_non_exhaustive()
    }
}

/// The server clock in milliseconds.
pub(crate) fn now_ms() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl BatchSink for Sink {
    fn record(
        &self,
        workspace: &str,
        actor: &str,
        call: &BatchCall,
        done: Option<&[(u64, bool)]>,
    ) -> Option<Marked> {
        let principal = Principal {
            workspace: workspace.to_owned(),
            actor: actor.to_owned(),
        };
        let seen: Vec<(u64, bool)> = match done {
            Some(d) => d.to_vec(),
            None => {
                let keys: Vec<String> = (0..call.commands)
                    .map(|i| format!("{}:{i}", call.key))
                    .collect();
                let found = self.reader.written(&principal, &call.file, &keys);
                let found = found.map_err(|e| eprintln!("meldframe-http-activity: {e:?}"));
                found.ok()?.into_iter().map(|s| (s, false)).collect()
            }
        };
        let (found, fresh) = (seen.len(), seen.iter().filter(|s| s.1).count());
        let first = seen.iter().map(|s| s.0).min()?;
        let last = seen.iter().map(|s| s.0).max()?;
        let found = u32::try_from(found).ok()?;
        let batch = NewBatch {
            actor: actor.to_owned(),
            key_prefix: call.key.clone(),
            tool: call.tool.into(),
            commands: u32::try_from(call.commands).ok()?,
            written: u32::try_from(fresh).ok()?,
            // The keys of the call the log holds: the count never passes it, and a replay racing the original it
            // replays (or a retry that fails again) is counted once, whichever reaches the store first.
            known: Some(found),
            atomic: call.atomic,
            first_seq: first,
            last_seq: last,
            at_ms: now_ms(),
            note: call.note.clone(),
        };
        let kept = |e| eprintln!("meldframe-http-activity: no header kept: {e}");
        let r = (self.store.record(workspace, &call.file, &batch))
            .map_err(kept)
            .ok()?;
        if let (true, Some(live)) = (r.changed, &self.live) {
            // After the call's op events (the call has returned from the room). A drop is silent: the list is the truth.
            live(
                &principal,
                &call.file,
                wire::frame(&call.file, &r.header, r.unreviewed),
            );
        }
        Some(Marked {
            n: r.header.n,
            written: found,
            note: r.note.as_str(),
        })
    }
}
