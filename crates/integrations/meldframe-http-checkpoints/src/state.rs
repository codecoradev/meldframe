//! `GET /v1/files/{file}/checkpoints/{n}/state` (ADR-0023): the file as it was at a checkpoint, `{"seq", "nodes"}`, worked
//! out by `state_at` of `meldframe-revert` over the ops of the log, so a restore still works after a reload.
//!
//! **Bounded.** The replay starts at the latest snapshot when its `seq` is at most the checkpoint's, else at 0. More than
//! [`StateLimits::replay`] ops (5,000) is `too_far`, refused before any op is read. A miss spends [`SCAN_COST`] calls of
//! the principal's budget before the snapshot is read, a hit one. The answer at a `(workspace, file, seq)` never changes,
//! so its text is cached under an entry bound and a byte bound.
//!
//! **A person's read**, under the gate of the create route (a listed actor and an `Auto` policy for `checkpoint_state`):
//! the state is the whole past of a file, deleted content included. The call is in the call log with its outcome code.

use crate::routes::{BAD_FILE, Fail, Routed};
use crate::{STATE_TOOL, StateLimits};
use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, header};
use axum::{body::Body, response::IntoResponse};
use meldframe_host::{Principal, REPLAY_PAGE, StoredOp};
use meldframe_http::{Failure, Reply, bearer, peer, valid_file};
use meldframe_protocol::Doc;
use meldframe_revert::{LoggedOp, state_at_doc};
use meldframe_text::canonical_u32;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

/// Calls of the principal's budget a replay costs; a cache hit costs one. The snapshot route's figure.
pub const SCAN_COST: u32 = 8;

/// What the replay reads of one opening of a file's log.
pub trait Log {
    /// The newest snapshot of the file: its `seq`, its nodes and its variables.
    fn snapshot(&self) -> Result<Option<(u64, Doc)>, Failure>;
    /// At most `limit` ops with a number above `after`, in order.
    fn ops_after(&self, after: u64, limit: usize) -> Result<Vec<StoredOp>, Failure>;
}

/// Read access to the log of a file, for the replay. The server implements it next to its readers: `with_log` runs `f`
/// on one opening of the file under one lock, so the pages of the log are read from one open room and not one each, and
/// nothing is worked out under the lock. It blocks, so the route calls it from `Shared::blocking` under the call's
/// slot. It never writes, and a file that does not exist reads as empty.
pub trait Past: Send + Sync {
    fn with_log(
        &self,
        who: &Principal,
        file: &str,
        f: &mut dyn FnMut(&dyn Log),
    ) -> Result<(), Failure>;
}

type Key = (String, String, u64);

#[derive(Debug, Default)]
struct Held {
    map: HashMap<Key, Arc<str>>,
    order: VecDeque<Key>,
    bytes: usize,
}

/// The cached answers, oldest out first, bounded by [`StateLimits`].
#[derive(Debug)]
pub(crate) struct StateCache {
    limits: StateLimits,
    held: Mutex<Held>,
}

impl StateCache {
    pub(crate) fn new(limits: StateLimits) -> Self {
        StateCache {
            limits,
            held: Mutex::default(),
        }
    }

    fn get(&self, key: &Key) -> Option<Arc<str>> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.map.get(key).cloned()
    }

    fn put(&self, key: Key, text: Arc<str>) {
        // An answer that cannot fit by itself is not kept, so it never empties the cache for nothing.
        if text.len() > self.limits.bytes {
            return;
        }
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if held.map.contains_key(&key) {
            return;
        }
        held.bytes += text.len();
        held.order.push_back(key.clone());
        held.map.insert(key, text);
        while held.map.len() > self.limits.entries || held.bytes > self.limits.bytes {
            let Some(oldest) = held.order.pop_front() else {
                break;
            };
            if let Some(gone) = held.map.remove(&oldest) {
                held.bytes -= gone.len();
            }
        }
    }
}

/// Why a replay gave no state.
enum Stopped {
    TooFar {
        ops: u64,
        limit: u64,
    },
    Read(Failure),
    /// The log does not reach the checkpoint or does not replay: the operator's to repair, never the caller's.
    Broken(String),
}

impl From<Failure> for Stopped {
    fn from(f: Failure) -> Self {
        Stopped::Read(f)
    }
}

/// What the replay starts from and the ops it applies.
struct Gathered {
    base: Doc,
    log: Vec<LoggedOp>,
}

/// The snapshot to start at and the ops after it up to `seq`, refused `TooFar` before any op is read.
fn gather(log: &dyn Log, seq: u64, limit: u64) -> Result<Gathered, Stopped> {
    let (from, base) = match log.snapshot()? {
        Some((at, doc)) if at <= seq => (at, doc),
        _ => (0, Doc::default()),
    };
    if seq - from > limit {
        return Err(Stopped::TooFar {
            ops: seq - from,
            limit,
        });
    }
    let mut kept = Vec::new();
    let mut after = from;
    while after < seq {
        let page = usize::try_from((seq - after).min(REPLAY_PAGE as u64)).unwrap_or(REPLAY_PAGE);
        let ops = log.ops_after(after, page)?;
        let Some(last) = ops.last().map(|op| op.seq) else {
            return Err(Stopped::Broken(format!("the log ends before op {seq}")));
        };
        after = last;
        let wanted = ops.into_iter().take_while(|op| op.seq <= seq);
        kept.extend(wanted.map(|op| LoggedOp {
            seq: op.seq,
            actor: op.actor,
            key: op.key,
            command: op.command,
        }));
    }
    Ok(Gathered { base, log: kept })
}

/// The text of the answer for the file as it was after op `seq`.
fn replay(
    past: &dyn Past,
    who: &Principal,
    file: &str,
    seq: u64,
    limit: u64,
) -> Result<String, Stopped> {
    let mut gathered = None;
    past.with_log(who, file, &mut |log| {
        gathered = Some(gather(log, seq, limit))
    })?;
    let Some(gathered) = gathered else {
        return Err(Stopped::Broken("the log was not opened".into()));
    };
    let Gathered { base, log } = gathered?;
    let nodes = state_at_doc(&base, &log, seq).map_err(|e| Stopped::Broken(format!("{e:?}")))?;
    serde_json::to_string(&serde_json::json!({"seq": seq, "nodes": nodes}))
        .map_err(|e| Stopped::Broken(e.to_string()))
}

/// A checkpoint number as a path spells it: [`canonical_u32`], as the review route reads a batch number, and from 1
/// (a row is never numbered 0, so `0` is refused as a bad number and not looked up).
fn number(text: &str) -> Option<u32> {
    canonical_u32(text).filter(|&n| n >= 1)
}

async fn run(s: &Routed, who: &Principal, file: &str, n: &str) -> Result<Arc<str>, Fail> {
    if !s.can_read_state(who) {
        let why = "this actor may not read a file at a checkpoint";
        return Err(Fail::new(StatusCode::FORBIDDEN, "forbidden", why, false));
    }
    if !valid_file(file) {
        return Err(Fail::invalid(BAD_FILE));
    }
    let n = number(n).ok_or_else(|| Fail::invalid("a checkpoint number is digits, from 1"))?;
    let (c, w, f) = (
        s.checkpoints.clone(),
        who.workspace.clone(),
        file.to_owned(),
    );
    let rows = s
        .shared
        .blocking(who, move || c.store().list(&w, &f))
        .await??;
    // A file of another workspace lists no row, so it is told from a missing number by nothing.
    let Some(row) = rows.iter().find(|r| r.n == n) else {
        let why = "the file has no checkpoint with this number in this workspace";
        return Err(Fail::new(StatusCode::NOT_FOUND, "not_found", why, false));
    };
    let key = (who.workspace.clone(), file.to_owned(), row.seq);
    if let Some(text) = s.cache.get(&key) {
        return Ok(text);
    }
    // Before the snapshot is read and before any op is: what the budget cannot cover is not read.
    (1..SCAN_COST).try_for_each(|_| s.shared.principal_ok(who))?;
    let (past, w, f, seq) = (s.gate.past.clone(), who.clone(), file.to_owned(), row.seq);
    let limit = s.checkpoints.limits().replay;
    let done = s
        .shared
        .blocking(who, move || replay(past.as_ref(), &w, &f, seq, limit))
        .await?;
    match done {
        Ok(text) => {
            let text: Arc<str> = text.into();
            s.cache.put(key, text.clone());
            Ok(text)
        }
        Err(Stopped::TooFar { ops, limit }) => {
            let why = format!(
                "this checkpoint is {ops} ops behind its nearest snapshot; the limit is {limit}"
            );
            Err(Fail::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "too_far",
                &why,
                false,
            ))
        }
        Err(Stopped::Read(failure)) => Err(Fail::from(Reply::failure(&failure))),
        Err(Stopped::Broken(why)) => {
            eprintln!(
                "meldframe-http-checkpoints: the log of a file does not give a checkpoint: {why}"
            );
            // The operator's to repair and no retry can help: the detail goes to the log, the caller hears a plain code.
            let said = "the server could not work out the file at this checkpoint";
            Err(Fail::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                said,
                false,
            ))
        }
    }
}

pub(crate) async fn state(
    State(s): State<Arc<Routed>>,
    Path((file, n)): Path<(String, String)>,
    req: Request<Body>,
) -> axum::response::Response {
    let who = s
        .shared
        .client_ok(&peer(&req))
        .and_then(|()| s.shared.principal(bearer(req.headers())));
    let who = match who {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    let done = run(&s, &who, &file, &n).await;
    // The log holds a file name only when it is one, and no number.
    let named = if valid_file(&file) { file.as_str() } else { "" };
    let code = done.as_ref().err().map(|f| f.code.clone());
    s.checkpoints
        .log(&s.shared, &who, STATE_TOOL, named, code)
        .await;
    match done {
        Ok(text) => {
            let json = [(header::CONTENT_TYPE, "application/json")];
            (StatusCode::OK, json, text.to_string()).into_response()
        }
        Err(f) => (f.status, axum::Json(f.body)).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::number;

    #[test]
    fn a_checkpoint_number_is_one_spelling_and_starts_at_one() {
        assert_eq!(number("1"), Some(1));
        assert_eq!(number("4294967295"), Some(u32::MAX));
        // Zero is a well-spelled number, but no checkpoint has it: the route answers `invalid`, not `not_found`.
        for bad in ["0", "01", "+1", "1 ", "", "4294967296", "x"] {
            assert_eq!(number(bad), None, "{bad:?}");
        }
    }
}
