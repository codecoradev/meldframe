//! `POST /captures/{requestId}`: the editor uploads the PNG it rendered.

use crate::hub::Hub;
use axum::{
    Router,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use capture::Done;
use futures_util::StreamExt;
use meldframe_http::{Failure, Reply, bearer, peer};
use serde_json::json;
use std::sync::Arc;

pub(crate) fn routes(hub: Arc<Hub>) -> Router {
    Router::new()
        .route("/captures/{request_id}", post(upload))
        .with_state(hub)
}

fn say(status: StatusCode, code: &str) -> Response {
    (
        status,
        axum::Json(json!({"ok": status.is_success(), "code": code})),
    )
        .into_response()
}

fn header_of<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name)?.to_str().ok()
}

/// The status for a finished request: 200 for an image taken, 413 for one over the limit, 422 for the rest.
fn answer(done: Option<Done>) -> Response {
    match done.map(|d| d.result) {
        None => say(StatusCode::NOT_FOUND, "not_found"),
        Some(Ok(_)) => say(StatusCode::OK, "ok"),
        Some(Err(e)) if e.code() == "too_large" => say(StatusCode::PAYLOAD_TOO_LARGE, "too_large"),
        Some(Err(e)) => say(StatusCode::UNPROCESSABLE_ENTITY, e.code()),
    }
}

async fn upload(
    State(hub): State<Arc<Hub>>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Response {
    let s = &hub.shared;
    let who = match s
        .client_ok(&peer(&req))
        .and_then(|()| s.principal(bearer(req.headers())))
    {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    let h = req.headers();
    let (Some(token), Some(seq)) = (
        header_of(h, "x-capture-token").map(str::to_owned),
        header_of(h, "x-capture-seq").and_then(|v| v.parse::<u64>().ok()),
    ) else {
        return Reply::bad_request("send X-Capture-Token and X-Capture-Seq (a sequence number)")
            .into_response();
    };
    // Before any body is read: only a pending request with this token gets one.
    let ws = who.workspace.clone();
    if !hub.with_relay(|r| r.is_pending(&ws, &id, &token)) {
        return answer(None);
    }
    let Some(_place) = hub.uploads.take(&who) else {
        return Reply::failure(&Failure::Unavailable("too many uploads".into())).into_response();
    };
    let max = hub.with_relay(|r| r.max_bytes());
    let declared =
        header_of(h, header::CONTENT_LENGTH.as_str()).and_then(|v| v.parse::<usize>().ok());
    let too_large = |hub: &Arc<Hub>| {
        let (ws, id, token) = (ws.clone(), id.clone(), token.clone());
        hub.run(move |r| r.refuse_too_large(&ws, &id, &token).into_iter().collect())
            .into_iter()
            .next()
    };
    if declared.is_some_and(|n| n > max) {
        return answer(too_large(&hub));
    }
    // Read at most max + 1 bytes: the extra byte tells an over-limit body from one at the limit.
    let mut body = Vec::with_capacity(declared.unwrap_or(0).min(max));
    let mut stream = req.into_body().into_data_stream();
    let read = tokio::time::timeout(s.body_timeout, async {
        while let Some(chunk) = stream.next().await {
            let Ok(chunk) = chunk else { return false };
            let room = (max + 1).saturating_sub(body.len());
            body.extend_from_slice(&chunk[..chunk.len().min(room)]);
            if body.len() > max {
                break;
            }
        }
        true
    })
    .await;
    match read {
        Err(_) => return say(StatusCode::REQUEST_TIMEOUT, "request_timeout"),
        Ok(false) => return say(StatusCode::BAD_REQUEST, "bad_request"),
        Ok(true) => {}
    }
    if body.len() > max {
        return answer(too_large(&hub));
    }
    let h2 = hub.clone();
    let done = s
        .blocking(&who, move || {
            h2.run(|r| r.upload(&ws, &id, &token, seq, &body).into_iter().collect())
        })
        .await;
    match done {
        Ok(done) => answer(done.into_iter().next()),
        Err(reply) => reply.into_response(),
    }
}
