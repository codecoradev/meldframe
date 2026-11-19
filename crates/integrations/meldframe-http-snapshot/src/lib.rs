//! `GET /v1/files/{file}`: the file as it is now, for a client that has no copy yet. It answers
//! `{"seq": n, "nodes": [...]}`; the client then subscribes to the live stream `after` that number. Each
//! node carries `lastActor` and `lastSeq`: the actor and the sequence number of the last op that touched it
//! (the same notion as `get_tree`: an add, update, remove, reparent or reorder names the node, a reparent
//! also its new parent; an undo is an ordinary op of whoever undoes). The log keeps no time, so there is no
//! `lastAt`.
//!
//! The attribution is read from the log, never from a map kept beside the room, so it is the same for a live
//! room, a room restored from the store (snapshot floor included) and a Postgres replay. It looks at the
//! last [`SCAN_WINDOW`] ops: a node not touched in them has neither field. The route builds on
//! `meldframe-http`: the same authenticator, per-address and per-principal budgets and in-flight slots
//! through [`Shared`]. The principal's workspace comes from its token, so a file of another workspace is
//! never read, and an unknown file is `not_found` (another workspace's file looks the same).

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use meldframe_batch::touches;
use meldframe_host::{Principal, REPLAY_PAGE, Refusal, Room, Store, StoreError};
use meldframe_http::{Failure, Reply, Shared, bearer, peer, valid_file};
use meldframe_protocol::{FileSnapshot, SnapshotNode};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

/// How many of the newest ops the attribution reads. A node untouched in them has no `lastActor`.
pub const SCAN_WINDOW: u64 = 5_000;

/// Files whose attribution is kept; past it the oldest entry is forgotten.
pub const MAX_CACHED_FILES: usize = 64;

/// Calls of the principal's budget a read that had to scan the log costs (a cache hit costs one).
pub const SCAN_COST: u32 = 8;

type Last = Arc<HashMap<String, (u64, String)>>;

/// The last op of each node, as worked out at one head.
#[derive(Debug)]
struct Entry {
    head: u64,
    window: u64,
    last: Last,
}

#[derive(Debug, Default)]
struct Inner {
    map: HashMap<(String, String), Entry>,
    order: VecDeque<(String, String)>,
}

/// The attribution of recent reads, per (workspace, file) and head sequence number. The log is append-only,
/// so the answer at a head never changes: a read at the same head does not page the log again, and a write
/// (a new head) makes the entry stale. Bounded by [`MAX_CACHED_FILES`].
#[derive(Debug, Default)]
pub struct AttributionCache(Mutex<Inner>);

impl AttributionCache {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Files held now.
    pub fn len(&self) -> usize {
        self.inner().map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get(&self, key: &(String, String), head: u64, window: u64) -> Option<Last> {
        let inner = self.inner();
        let e = inner.map.get(key)?;
        (e.head == head && e.window == window).then(|| e.last.clone())
    }

    fn put(&self, key: (String, String), entry: Entry) {
        let mut inner = self.inner();
        if inner.map.insert(key.clone(), entry).is_none() {
            inner.order.push_back(key);
        }
        while inner.map.len() > MAX_CACHED_FILES {
            let Some(oldest) = inner.order.pop_front() else {
                break;
            };
            inner.map.remove(&oldest);
        }
    }
}

/// A snapshot, and whether the log had to be scanned for it (false: the cache answered).
#[derive(Debug, Clone)]
pub struct Read {
    pub snapshot: FileSnapshot,
    pub scanned: bool,
}

/// The newest op of the last `window` that touched each node.
fn scan<S: Store>(room: &Room<S>, window: u64) -> Result<Last, StoreError> {
    let head = room.seq();
    let mut last: HashMap<String, (u64, String)> = HashMap::new();
    let mut after = head.saturating_sub(window);
    while after < head {
        let page = room.store().ops_after(room.id(), after, REPLAY_PAGE)?;
        let Some(newest) = page.last().map(|op| op.seq) else {
            break;
        };
        for op in page.into_iter().filter(|op| op.seq <= head) {
            for id in touches(&op.command) {
                last.insert(id, (op.seq, op.actor.clone()));
            }
        }
        after = newest;
    }
    Ok(Arc::new(last))
}

fn build<S: Store>(room: &Room<S>, last: &Last) -> FileSnapshot {
    let nodes = room
        .nodes()
        .iter()
        .map(|node| {
            let found = last.get(&node.id);
            SnapshotNode {
                node: node.clone(),
                last_actor: found.map(|(_, actor)| actor.clone()),
                last_seq: found.map(|(seq, _)| *seq),
            }
        })
        .collect();
    FileSnapshot {
        seq: room.seq(),
        nodes,
        variables: room.vars().clone(),
    }
}

/// The file of `room` with the last actor of each node, from the newest `window` ops of its log. `None`
/// when the file has no ops: it does not exist (nothing is written for a name that was only read).
pub fn snapshot_of<S: Store>(
    room: &Room<S>,
    window: u64,
) -> Result<Option<FileSnapshot>, StoreError> {
    if room.seq() == 0 {
        return Ok(None);
    }
    Ok(Some(build(room, &scan(room, window)?)))
}

/// [`snapshot_of`] through `cache`: at a head the cache holds, the log is not read.
pub fn snapshot_cached<S: Store>(
    room: &Room<S>,
    window: u64,
    cache: &AttributionCache,
) -> Result<Option<Read>, StoreError> {
    let head = room.seq();
    if head == 0 {
        return Ok(None);
    }
    let key = (room.workspace().to_owned(), room.id().to_owned());
    let (last, scanned) = match cache.get(&key, head, window) {
        Some(last) => (last, false),
        None => {
            let last = scan(room, window)?;
            let entry = Entry {
                head,
                window,
                last: last.clone(),
            };
            cache.put(key, entry);
            (last, true)
        }
    };
    Ok(Some(Read {
        snapshot: build(room, &last),
        scanned,
    }))
}

/// Read access to a file's current state. The server implements it with [`snapshot_cached`] and the cache the
/// route passes. The implementation blocks, so the route calls it from `spawn_blocking` under the call's
/// slot. `None` is a file that does not exist in the principal's workspace.
pub trait Snapshots: Send + Sync {
    fn snapshot(
        &self,
        who: &Principal,
        file: &str,
        cache: &AttributionCache,
    ) -> Result<Option<Read>, Failure>;
}

struct Routed {
    shared: Arc<Shared>,
    source: Arc<dyn Snapshots>,
    cache: AttributionCache,
}

/// Adds `GET /v1/files/{file}` to the routes of a `Shared`.
pub fn routes(shared: Arc<Shared>, source: Arc<dyn Snapshots>) -> Router {
    Router::new()
        .route("/v1/files/{file}", get(snapshot))
        .with_state(Arc::new(Routed {
            shared,
            source,
            cache: AttributionCache::default(),
        }))
}

/// The error shape of the other routes (`{"error": {"code", "message", "retryable"}}`) with status 404.
fn not_found() -> Response {
    let body = json!({"error": {
        "code": "not_found",
        "message": "the file does not exist in this workspace",
        "retryable": false,
    }});
    (StatusCode::NOT_FOUND, Json(body)).into_response()
}

async fn snapshot(
    State(s): State<Arc<Routed>>,
    Path(file): Path<String>,
    req: Request<Body>,
) -> Response {
    let who = match s
        .shared
        .client_ok(&peer(&req))
        .and_then(|()| s.shared.principal(bearer(req.headers())))
    {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    if !valid_file(&file) {
        let reason = "a file name is 1 to 128 bytes without slashes or control characters".into();
        return Reply::failure(&Failure::Refused(Refusal::Invalid { reason })).into_response();
    }
    let (routed, who2) = (s.clone(), who.clone());
    let done = s
        .shared
        .blocking(&who, move || {
            routed.source.snapshot(&who2, &file, &routed.cache)
        })
        .await;
    match done {
        Err(reply) => reply.into_response(),
        Ok(Err(failure)) => {
            if let Failure::Unavailable(why) | Failure::Permanent(why) = &failure {
                eprintln!("meldframe-http-snapshot: unavailable: {why:?}");
            }
            Reply::failure(&failure).into_response()
        }
        Ok(Ok(None)) => not_found(),
        Ok(Ok(Some(read))) => {
            if read.scanned {
                // The scan held the server lock for a paged read of the log: it costs more than one call.
                // What the budget cannot cover is not charged; the next call is the one that is limited.
                let _ = s.shared.principal_wait(&who, SCAN_COST - 1);
            }
            (StatusCode::OK, Json(read.snapshot)).into_response()
        }
    }
}
