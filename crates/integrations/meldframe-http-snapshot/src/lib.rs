//! `GET /v1/files/{file}`: the file as it is now, for a client that has no copy yet. It answers
//! `{"seq": n, "nodes": [...]}`; the client then subscribes to the live stream `after` that number. It
//! builds on `meldframe-http`: the same authenticator, per-address and per-principal budgets and in-flight
//! slots through [`Shared`]. The principal's workspace comes from its token, so a file of another
//! workspace is never read.

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use meldframe_host::{Principal, Refusal};
use meldframe_http::{Failure, Reply, Shared, bearer, peer, valid_file};
use meldframe_protocol::Node;
use serde_json::json;
use std::sync::Arc;

/// A file at one sequence number.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub seq: u64,
    pub nodes: Vec<Node>,
}

/// Read access to a file's current state. The server implements it. The implementation blocks, so the
/// route calls it from `spawn_blocking` under the call's slot. A file that does not exist reads as
/// sequence 0 and empty.
pub trait Snapshots: Send + Sync {
    fn snapshot(&self, who: &Principal, file: &str) -> Result<FileSnapshot, Failure>;
}

struct State_ {
    shared: Arc<Shared>,
    source: Arc<dyn Snapshots>,
}

/// Adds `GET /v1/files/{file}` to the routes of a `Shared`.
pub fn routes(shared: Arc<Shared>, source: Arc<dyn Snapshots>) -> Router {
    Router::new()
        .route("/v1/files/{file}", get(snapshot))
        .with_state(Arc::new(State_ { shared, source }))
}

async fn snapshot(
    State(s): State<Arc<State_>>,
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
        Ok(Ok(snap)) => (
            StatusCode::OK,
            Json(json!({"seq": snap.seq, "nodes": snap.nodes})),
        )
            .into_response(),
    }
}
