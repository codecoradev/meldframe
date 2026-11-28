//! The upload and the download of one image: `POST /v1/assets` and `GET /v1/assets/{id}`.

use super::failure::failure;
use super::state::{Refused, State_};
use crate::{Failure, Reply};
use assets::{AssetError, ImageType, MAX_ASSET_BYTES};
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use meldframe_host::Refusal;
use meldframe_protocol::is_asset_id;
use serde_json::json;
use std::sync::Arc;

pub(super) async fn upload(State(s): State<Arc<State_>>, req: Request<Body>) -> Response {
    let who = match s.who(&req) {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    let declared = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|n| n > MAX_ASSET_BYTES) {
        return failure(&AssetError::TooLarge {
            limit: MAX_ASSET_BYTES,
        });
    }
    let Some(_place) = s.uploads.take(&who) else {
        return Reply::failure(&Failure::Unavailable("too many uploads".into())).into_response();
    };
    // At most the limit plus one byte: the extra byte tells an over-limit body from one at the limit.
    let mut body = Vec::with_capacity(declared.unwrap_or(0).min(MAX_ASSET_BYTES));
    let mut stream = req.into_body().into_data_stream();
    let read = tokio::time::timeout(s.upload_timeout, async {
        while let Some(chunk) = stream.next().await {
            let Ok(chunk) = chunk else { return false };
            let room = (MAX_ASSET_BYTES + 1).saturating_sub(body.len());
            body.extend_from_slice(&chunk[..chunk.len().min(room)]);
            if body.len() > MAX_ASSET_BYTES {
                break;
            }
        }
        true
    })
    .await;
    match read {
        Err(_) => {
            return (
                StatusCode::REQUEST_TIMEOUT,
                axum::Json(json!({"error": {"code": "request_timeout",
                    "message": "the upload took too long", "retryable": true}})),
            )
                .into_response();
        }
        Ok(false) => return Reply::bad_request("the body could not be read").into_response(),
        Ok(true) => {}
    }
    // Past the limit: no need to ask the store to say so.
    if body.len() > MAX_ASSET_BYTES {
        return failure(&AssetError::TooLarge {
            limit: MAX_ASSET_BYTES,
        });
    }
    let (state, ws) = (s.clone(), who.workspace.clone());
    let done = s
        .shared
        .blocking(&who, move || state.store(&ws, &body))
        .await;
    match done {
        Err(reply) => reply.into_response(),
        Ok(Err(Refused::Asset(e))) => failure(&e),
        Ok(Err(Refused::Storage(limit))) => {
            Reply::failure(&Failure::Refused(Refusal::StorageExceeded { limit })).into_response()
        }
        Ok(Ok(stored)) => axum::Json(json!({"ok": true, "id": stored.id,
            "type": stored.kind.mime(), "bytes": stored.bytes}))
        .into_response(),
    }
}

pub(super) async fn download(
    State(s): State<Arc<State_>>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Response {
    let who = match s.who(&req) {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    if !is_asset_id(&id) {
        return failure(&AssetError::Invalid(
            "an asset id is a lowercase hex SHA-256 (64 characters)".into(),
        ));
    }
    let (state, ws) = (s.clone(), who.workspace.clone());
    let got = s
        .shared
        .blocking(&who, move || state.blobs.get(&ws, &id))
        .await;
    match got {
        Err(reply) => reply.into_response(),
        Ok(Err(e)) => failure(&e),
        Ok(Ok((kind, bytes))) => served(kind, bytes),
    }
}

/// The bytes with what a browser needs to treat them as an image and nothing else. The asset is named by its
/// content, so it never changes (cacheable for a year), and it is private to the workspace.
fn served(kind: ImageType, bytes: Vec<u8>) -> Response {
    let mut r = Response::new(Body::from(bytes));
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind.mime()));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    // The asset belongs to the workspace of the token: a cache must not give it to another one.
    h.insert(header::VARY, HeaderValue::from_static("Authorization"));
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    r
}
