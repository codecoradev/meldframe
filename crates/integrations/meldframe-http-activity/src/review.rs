//! The review setter: `PUT /v1/files/{file}/batches/{n}/review` with `{"state": "unreviewed" | "accepted" |
//! "needs_review"}` (ADR-0021). It is a person's decision, so it is a route and not an MCP tool: the catalogue lists
//! no setter and an agent cannot accept its own work through the surface agents use.
//!
//! It takes the path of every Change call: the per-address bucket, the token, the per-principal bucket, the call's
//! slot and time limit, and it is policed and logged. The registry has it as [`REVIEW_TOOL`], a Change, so a
//! deployment can deny it and the call log records its tier. Who may use it is the list [`Reviewers`] (exact match,
//! deny by default) and the registry's policy, both: a call that fails either is `forbidden`. Who set the state and
//! when come from the token and the server clock, never from the request. The call log holds the actor, the tool, the
//! file and the outcome code and nothing else: no batch number and no state.
//!
//! The answer is `{"batch": <row>, "unreviewedCount": n}`. Setting the state a row has is a 200 that changes nothing,
//! takes no `rev` and sends no frame; a change is queued as a `batch` frame to every stream of the file.

use crate::reviewers::Reviewers;
use crate::route::Routed;
use crate::service::Publish;
use crate::sink::now_ms;
use crate::wire::{frame, row};
use activity_store::ActivityError;
use axum::extract::{Path, Request, State};
use axum::{Json, body::Body, http::StatusCode, response::IntoResponse};
use meldframe_activity::Review;
use meldframe_host::Principal;
use meldframe_http::mcp::Reader;
use meldframe_http::{Reply, Shared, bearer, peer, valid_file};
use meldframe_module::Policy;
use meldframe_text::canonical_u32;
use serde_json::{Value, json};
use std::sync::Arc;

/// The name the setter has in the registry and in the call log.
pub const REVIEW_TOOL: &str = "batch_review";

/// The registry's text for the setter. It is not an MCP tool, so no client reads it.
pub const REVIEW_DESCRIPTION: &str = "Set the review state of a batch of a file: unreviewed, accepted or needs_review. A person's decision, made through the route PUT /v1/files/{file}/batches/{n}/review by an actor listed in MELDFRAME_REVIEWERS; not offered over MCP.";

/// The body of the request is a state and nothing else: far below the server's body limit.
const MAX_BODY: usize = 1024;

/// Who may set a review state, what the registry's policy says, and what a change needs to be logged and sent.
pub(crate) struct Setter {
    reviewers: Reviewers,
    policy: Option<Policy>,
    reader: Arc<dyn Reader>,
    live: Option<Publish>,
}

impl std::fmt::Debug for Setter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Setter")
            .field("reviewers", &self.reviewers.len())
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl Setter {
    /// A setter that nobody may use.
    pub(crate) fn new(reader: Arc<dyn Reader>, live: Option<Publish>) -> Self {
        Setter {
            reviewers: Reviewers::default(),
            policy: None,
            reader,
            live,
        }
    }

    pub(crate) fn with(&self, reviewers: Reviewers, policy: Option<Policy>) -> Self {
        Setter {
            reviewers,
            policy,
            reader: self.reader.clone(),
            live: self.live.clone(),
        }
    }

    /// Whether the caller may set a review state: listed in `MELDFRAME_REVIEWERS` and not denied by the registry's
    /// policy for the setter. The one rule: the route refuses on it and the list route's `canReview` says it, so
    /// the two cannot drift.
    pub(crate) fn can_review(&self, who: &Principal) -> bool {
        self.reviewers.contains(&who.actor) && self.policy == Some(Policy::Auto)
    }

    /// One entry of the call log: in a slot of the principal and under the time limit, like every other call (a
    /// busy server loses the entry rather than holding the answer).
    async fn log(&self, shared: &Shared, who: &Principal, file: &str, code: Option<String>) {
        let (reader, w, file) = (self.reader.clone(), who.clone(), file.to_owned());
        let job = move || reader.record(&w, REVIEW_TOOL, &file, code.as_deref());
        let _ = shared.blocking(who, job).await;
    }
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

/// The state a request names, or why not.
fn state(bytes: &[u8]) -> Result<Review, Fail> {
    let form = "the body must be a JSON object {\"state\": \"unreviewed\", \"accepted\" or \"needs_review\"}";
    let Ok(Value::Object(body)) = serde_json::from_slice::<Value>(bytes) else {
        return Err(Fail::new(
            StatusCode::BAD_REQUEST,
            "bad_request",
            form,
            false,
        ));
    };
    let named = body.get("state").and_then(Value::as_str);
    let found = named.and_then(Review::parse);
    match (found, body.len()) {
        (Some(r), 1) => Ok(r),
        _ => Err(Fail::invalid(form)),
    }
}

async fn run(
    s: &Routed,
    who: &Principal,
    file: &str,
    n: &str,
    req: Request<Body>,
) -> Result<Value, Fail> {
    if !s.setter.can_review(who) {
        let why = "this actor may not set a review state";
        return Err(Fail::new(StatusCode::FORBIDDEN, "forbidden", why, false));
    }
    let read = axum::body::to_bytes(req.into_body(), MAX_BODY);
    let bytes = match tokio::time::timeout(s.shared.body_timeout, read).await {
        Ok(Ok(bytes)) => bytes,
        // As the other routes answer them: a body that does not arrive, and one that is over the limit.
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
    let wanted = state(&bytes)?;
    if !valid_file(file) {
        return Err(Fail::invalid(
            "a file name is 1 to 128 bytes without slashes, control or invisible formatting characters",
        ));
    }
    // One spelling for each number, so `+1` and `01` do not alias 1.
    let n = canonical_u32(n).ok_or_else(|| Fail::invalid("a batch number is a whole number"))?;
    let (store, live, p, file) = (
        s.store.clone(),
        s.setter.live.clone(),
        who.clone(),
        file.to_owned(),
    );
    let job = move || {
        let r = store.set_review(&p.workspace, &file, n, wanted, &p.actor, now_ms())?;
        if let (true, Some(live)) = (r.changed, live) {
            live(&p, &file, frame(&file, &r.header, r.unreviewed));
        }
        Ok(r)
    };
    let done = s.shared.blocking(who, job).await?;
    match done {
        Ok(r) => Ok(json!({"batch": row(&r.header), "unreviewedCount": r.unreviewed})),
        Err(ActivityError::NotFound(_)) => Err(Fail::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "this file has no such batch",
            false,
        )),
        Err(ActivityError::Invalid(why)) => Err(Fail::invalid(&why)),
        Err(other) => {
            eprintln!("meldframe-http-activity: review: {other}");
            let why = "the server cannot take this call now; try again later";
            Err(Fail::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                why,
                true,
            ))
        }
    }
}

pub(crate) async fn put(
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
    let done = run(&s, &who, &file, &n, req).await;
    // The log holds a file name only when it is one.
    let named = if valid_file(&file) { file.as_str() } else { "" };
    let code = done.as_ref().err().map(|f| f.code.clone());
    s.setter.log(&s.shared, &who, named, code).await;
    match done {
        Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        Err(f) => (f.status, Json(f.body)).into_response(),
    }
}
