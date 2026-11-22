//! `GET` and `POST /v1/files/{file}/checkpoints` (ADR-0023). Both take the path of the snapshot route: the per-address
//! bucket, the token, the per-principal bucket, the call's slot and time limit, `valid_file`, and the workspace of the
//! principal. Neither takes a workspace or an actor from a request.
//!
//! The list answers `{"checkpoints": [...], "canCreate": bool}`, newest `n` first; any principal of the workspace may
//! read it, a file with none is an empty list and not `not_found` (so another workspace's file is not told from an
//! empty one), and, like the list of batches, it has no entry of its own in the call log.
//!
//! The create is a person's call, so it is a route and not an MCP tool. Who may make a row is the closure `may` (the
//! actors of `MELDFRAME_REVIEWERS` until a principal has a kind) and the registry's policy for [`CREATE_TOOL`], both;
//! a call that fails either is `forbidden`, and `canCreate` of the list is the same rule. The body is
//! `{"key", "kind", "seq"?, "label"?, "about"?}`: unknown fields are refused, `by` is the principal's actor and never
//! a field, `runStart` is the server's and refused, `seq` is at most the head of the file and defaults to it, and a file
//! with no ops has nothing to checkpoint. The call log holds the actor, the tool, the file and the outcome code and
//! nothing else: no label.
//!
//! The answer is `{"checkpoint": <row>, "created": bool}`. The same request again answers its row with `created`
//! false and sends no frame; the same key with another request is `key_reused`, whose error carries the first row's `n`; a row that was made is queued as a
//! `checkpoint` frame to every stream of the file.

use crate::wire::row;
use crate::{CREATE_TOOL, Checkpoints, Gate};
use axum::extract::{Path, Request, State};
use axum::routing::get;
use axum::{Json, Router, body::Body, http::StatusCode, response::IntoResponse};
use checkpoint_store::CheckpointError;
use meldframe_checkpoints::{Kind, Label, NewCheckpoint};
use meldframe_host::Principal;
use meldframe_http::{Reply, Shared, bearer, peer, valid_file};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

/// The body is a few short fields: far below the server's body limit.
const MAX_BODY: usize = 2048;

pub(crate) struct Routed {
    pub(crate) shared: Arc<Shared>,
    pub(crate) checkpoints: Arc<Checkpoints>,
    pub(crate) gate: Gate,
}

impl Routed {
    /// Whether the caller may make a row: listed and not denied by the registry's policy. The one rule: the create
    /// refuses on it and the list's `canCreate` says it, so the two cannot drift.
    fn can_create(&self, who: &Principal) -> bool {
        (self.gate.may)(&who.actor) && self.gate.policy == Some(meldframe_module::Policy::Auto)
    }
}

/// Adds the two routes to the routes of a `Shared`.
pub(crate) fn routes(shared: Arc<Shared>, checkpoints: Arc<Checkpoints>, gate: Gate) -> Router {
    let state = Routed {
        shared,
        checkpoints,
        gate,
    };
    Router::new()
        .route("/v1/files/{file}/checkpoints", get(list).post(create))
        .with_state(Arc::new(state))
}

/// A failed call: the status, the stable code and the JSON body.
struct Fail {
    status: StatusCode,
    code: String,
    body: Value,
}

impl Fail {
    fn new(status: StatusCode, code: &str, message: &str, retryable: bool) -> Self {
        let error = json!({"code": code, "message": message, "retryable": retryable});
        Fail {
            status,
            code: code.to_owned(),
            body: json!({"error": error}),
        }
    }

    fn invalid(message: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid", message, false)
    }
}

impl From<Reply> for Fail {
    fn from(r: Reply) -> Self {
        let code = r.body["error"]["code"].as_str().unwrap_or("internal");
        Fail {
            status: r.status,
            code: code.to_owned(),
            body: r.body,
        }
    }
}

impl From<CheckpointError> for Fail {
    fn from(e: CheckpointError) -> Self {
        let message = e.to_string();
        match e {
            CheckpointError::Invalid(_) => Fail::invalid(&message),
            CheckpointError::Full { .. } => {
                Fail::new(StatusCode::CONFLICT, "checkpoint_full", &message, false)
            }
            CheckpointError::KeyReused(n) => {
                // The first row's number is a field a client reads, not only words.
                let mut f = Fail::new(StatusCode::CONFLICT, "key_reused", &message, false);
                f.body["error"]["n"] = json!(n);
                f
            }
            CheckpointError::Unavailable(why) => {
                eprintln!("meldframe-http-checkpoints: {why}");
                let why = "the server cannot take this call now; try again later";
                Fail::new(StatusCode::SERVICE_UNAVAILABLE, "unavailable", why, true)
            }
        }
    }
}

const BAD_FILE: &str =
    "a file name is 1 to 128 bytes without slashes, control or invisible formatting characters";

async fn list(
    State(s): State<Arc<Routed>>,
    Path(file): Path<String>,
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
    if !valid_file(&file) {
        return Reply::failure(&meldframe_http::Failure::Refused(
            meldframe_host::Refusal::Invalid {
                reason: BAD_FILE.into(),
            },
        ))
        .into_response();
    }
    let (c, workspace) = (s.checkpoints.clone(), who.workspace.clone());
    let done = s
        .shared
        .blocking(&who, move || c.store().list(&workspace, &file))
        .await;
    match done {
        Err(reply) => reply.into_response(),
        Ok(Err(e)) => {
            let f = Fail::from(e);
            (f.status, Json(f.body)).into_response()
        }
        Ok(Ok(rows)) => {
            let checkpoints: Vec<_> = rows.iter().map(row).collect();
            let body = json!({"checkpoints": checkpoints, "canCreate": s.can_create(&who)});
            (StatusCode::OK, Json(body)).into_response()
        }
    }
}

/// What a create may carry. `by` is not here, so a body that sends it is refused as an unknown field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wanted {
    key: String,
    kind: String,
    seq: Option<u64>,
    label: Option<String>,
    about: Option<String>,
}

async fn read_body(s: &Routed, req: Request<Body>) -> Result<Wanted, Fail> {
    let read = axum::body::to_bytes(req.into_body(), MAX_BODY);
    let bytes = match tokio::time::timeout(s.shared.body_timeout, read).await {
        Ok(Ok(bytes)) => bytes,
        Err(_) => {
            let why = "the request body took too long to arrive";
            return Err(Fail::new(
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
                why,
                true,
            ));
        }
        Ok(Err(_)) => {
            let why = format!("the request is larger than {MAX_BODY} bytes");
            return Err(Fail::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                &why,
                false,
            ));
        }
    };
    let form =
        "the body must be a JSON object {\"key\", \"kind\", \"seq\"?, \"label\"?, \"about\"?}";
    let value = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .filter(Value::is_object);
    let Some(value) = value else {
        return Err(Fail::new(
            StatusCode::BAD_REQUEST,
            "bad_request",
            form,
            false,
        ));
    };
    serde_json::from_value(value).map_err(|e| Fail::invalid(&format!("{form}: {e}")))
}

async fn run(
    s: &Routed,
    who: &Principal,
    file: &str,
    req: Request<Body>,
) -> Result<(Value, bool), Fail> {
    if !s.can_create(who) {
        let why = "this actor may not make a checkpoint";
        return Err(Fail::new(StatusCode::FORBIDDEN, "forbidden", why, false));
    }
    let body = read_body(s, req).await?;
    if !valid_file(file) {
        return Err(Fail::invalid(BAD_FILE));
    }
    let kind = Kind::parse(&body.kind)
        .ok_or_else(|| Fail::invalid("kind is manual, beforeRevert or beforeRestore"))?;
    let label =
        Label::parse_arg(body.label.as_deref()).map_err(|e| Fail::invalid(&e.to_string()))?;
    let shaped =
        |seq| NewCheckpoint::by_person(kind, seq, label.clone(), body.about.clone(), &who.actor);
    shaped(0).map_err(|e| Fail::invalid(&e.to_string()))?;
    // The head of the file, a read that is not a call of its own; a file with no ops has nothing to point at.
    let (c, w, f) = (s.checkpoints.clone(), who.clone(), file.to_owned());
    let head = s.shared.blocking(who, move || c.head(&w, &f)).await?;
    let head = head.map_err(|failure| Fail::from(Reply::failure(&failure)))?;
    if head == 0 {
        return Err(Fail::invalid(
            "nothing to checkpoint yet: this file has no ops",
        ));
    }
    let seq = body.seq.unwrap_or(head);
    if seq > head {
        return Err(Fail::invalid(&format!(
            "seq is at most the head of the file, {head}"
        )));
    }
    let new = shaped(seq).map_err(|e| Fail::invalid(&e.to_string()))?;
    let (c, w, f, key) = (
        s.checkpoints.clone(),
        who.clone(),
        file.to_owned(),
        body.key,
    );
    let job = move || {
        let made = c
            .store()
            .create(&w.workspace, &f, Some(&key), &new, c.now())?;
        if made.created {
            c.announce(&w, &f, &made.checkpoint);
        }
        Ok::<_, CheckpointError>(made)
    };
    let made = s.shared.blocking(who, job).await??;
    Ok((
        json!({"checkpoint": row(&made.checkpoint), "created": made.created}),
        made.created,
    ))
}

async fn create(
    State(s): State<Arc<Routed>>,
    Path(file): Path<String>,
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
    let done = run(&s, &who, &file, req).await;
    // The log holds a file name only when it is one, and no label.
    let named = if valid_file(&file) { file.as_str() } else { "" };
    let code = done.as_ref().err().map(|f| f.code.clone());
    s.checkpoints
        .log(&s.shared, &who, CREATE_TOOL, named, code)
        .await;
    match done {
        Ok((body, _)) => (StatusCode::OK, Json(body)).into_response(),
        Err(f) => (f.status, Json(f.body)).into_response(),
    }
}
