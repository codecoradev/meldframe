//! `GET /v1/files/{file}/batches?before={n}&after={n}&limit={1..100}`: the headers of a file, newest first (ADR-0021).
//! It answers `{"batches": [...], "hasMore": bool, "unreviewedCount": count, "canReview": bool}`. `before` and `after`
//! are exclusive and may be combined; a file with no batches is an empty list, not `not_found`, so a name in another
//! workspace is not told from one that is empty. It takes the path of the snapshot route: the per-address bucket, the
//! token, the per-principal bucket, the call's slot and time limit, `valid_file`, and the workspace of the principal.
//! The live `batch` frame is a hint over this list: a client that missed one reads it here.

use crate::wire::row;
use activity_store::{ActivityError, BatchStore, Query};
use axum::extract::{Path, Request, State};
use axum::{Json, Router, body::Body, http::StatusCode, response::IntoResponse, routing::get};
use meldframe_activity::{DEFAULT_PAGE, MAX_PAGE};
use meldframe_host::Refusal;
use meldframe_http::{Failure, Reply, Shared, bearer, peer, valid_file};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Whether the caller may set a review state. Nobody may until the setter exists (slice 7 of ADR-0021).
const CAN_REVIEW: bool = false;

struct Routed {
    shared: Arc<Shared>,
    store: Arc<dyn BatchStore>,
}

/// Adds `GET /v1/files/{file}/batches` to the routes of a `Shared`.
pub fn routes(shared: Arc<Shared>, store: Arc<dyn BatchStore>) -> Router {
    Router::new()
        .route("/v1/files/{file}/batches", get(list))
        .with_state(Arc::new(Routed { shared, store }))
}

fn invalid(reason: String) -> Failure {
    Failure::Refused(Refusal::Invalid { reason })
}

/// An optional number of the query: absent is `None`, anything that is not a whole number is refused.
fn number<T: std::str::FromStr>(
    p: &HashMap<String, String>,
    name: &str,
) -> Result<Option<T>, Failure> {
    let parse = |v: &String| {
        v.parse()
            .map_err(|_| invalid(format!("{name} is a whole number")))
    };
    p.get(name).map(parse).transpose()
}

/// The query string as name and value pairs. A string that does not parse is the same `invalid` as a bad number, in the
/// JSON shape of the other routes (the framework's extractor would answer a plain-text 400 of its own).
fn params(req: &Request<Body>) -> Result<HashMap<String, String>, Failure> {
    serde_urlencoded::from_str(req.uri().query().unwrap_or_default())
        .map_err(|_| invalid("the query string is not valid".into()))
}

fn query(p: &HashMap<String, String>) -> Result<Query, Failure> {
    let limit = number::<usize>(p, "limit")?.unwrap_or(DEFAULT_PAGE);
    if !(1..=MAX_PAGE).contains(&limit) {
        return Err(invalid(format!("limit is 1 to {MAX_PAGE}")));
    }
    Ok(Query {
        before: number(p, "before")?,
        after: number(p, "after")?,
        limit,
    })
}

async fn list(
    State(s): State<Arc<Routed>>,
    Path(file): Path<String>,
    req: Request<Body>,
) -> axum::response::Response {
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
        return Reply::failure(&invalid(reason)).into_response();
    }
    let q = match params(&req).and_then(|p| query(&p)) {
        Ok(q) => q,
        Err(f) => return Reply::failure(&f).into_response(),
    };
    let (store, workspace) = (s.store.clone(), who.workspace.clone());
    let done = s
        .shared
        .blocking(&who, move || store.list(&workspace, &file, &q))
        .await;
    match done {
        Err(reply) => reply.into_response(),
        Ok(Err(e)) => {
            let failure = match e {
                ActivityError::Invalid(why) => invalid(why),
                other => {
                    eprintln!("meldframe-http-activity: list: {other}");
                    Failure::Unavailable(other.to_string())
                }
            };
            Reply::failure(&failure).into_response()
        }
        Ok(Ok(page)) => {
            let batches: Vec<_> = page.headers.iter().map(row).collect();
            let body = json!({
                "batches": batches,
                "hasMore": page.has_more,
                "unreviewedCount": page.unreviewed,
                "canReview": CAN_REVIEW,
            });
            (StatusCode::OK, Json(body)).into_response()
        }
    }
}
