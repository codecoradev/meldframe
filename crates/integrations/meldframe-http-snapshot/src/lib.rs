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
use std::collections::HashMap;
use std::sync::Arc;

/// How many of the newest ops the attribution reads. A node untouched in them has no `lastActor`.
pub const SCAN_WINDOW: u64 = 5_000;

/// The file of `room` with the last actor of each node, from the newest `window` ops of its log. `None`
/// when the file has no ops: it does not exist (nothing is written for a name that was only read).
pub fn snapshot_of<S: Store>(
    room: &Room<S>,
    window: u64,
) -> Result<Option<FileSnapshot>, StoreError> {
    let head = room.seq();
    if head == 0 {
        return Ok(None);
    }
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
    Ok(Some(FileSnapshot { seq: head, nodes }))
}

/// Read access to a file's current state. The server implements it. The implementation blocks, so the
/// route calls it from `spawn_blocking` under the call's slot. `None` is a file that does not exist in the
/// principal's workspace.
pub trait Snapshots: Send + Sync {
    fn snapshot(&self, who: &Principal, file: &str) -> Result<Option<FileSnapshot>, Failure>;
}

struct Routed {
    shared: Arc<Shared>,
    source: Arc<dyn Snapshots>,
}

/// Adds `GET /v1/files/{file}` to the routes of a `Shared`.
pub fn routes(shared: Arc<Shared>, source: Arc<dyn Snapshots>) -> Router {
    Router::new()
        .route("/v1/files/{file}", get(snapshot))
        .with_state(Arc::new(Routed { shared, source }))
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
    let (source, who2) = (s.source.clone(), who.clone());
    match s
        .shared
        .blocking(&who, move || source.snapshot(&who2, &file))
        .await
    {
        Err(reply) => reply.into_response(),
        Ok(Err(failure)) => Reply::failure(&failure).into_response(),
        Ok(Ok(None)) => not_found(),
        Ok(Ok(Some(snap))) => (StatusCode::OK, Json(snap)).into_response(),
    }
}
