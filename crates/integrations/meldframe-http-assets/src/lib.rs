//! The asset routes (E2.5, ADR-0015). Both answer to the workspace of the Bearer token and to no other:
//!
//! - `POST /v1/assets`: the body is the bytes of one png, jpeg, webp or gif image (no multipart). The route
//!   checks the token and the declared length first, takes an upload place, reads the body as a stream and never
//!   past the limit, and stores it. The answer names the asset: `{"ok": true, "id": <SHA-256>, "type": "image/png",
//!   "bytes": n}`. The same bytes are the same asset.
//! - `GET /v1/assets/{id}`: the bytes, served with the type told by the first bytes, `X-Content-Type-Options:
//!   nosniff` and a content security policy that allows no script, since only the first bytes of an upload are
//!   checked. An id that is not in the workspace's store is `asset_not_found`, whoever else has it.
//!
//! An error is `{"error": {"code", "message", "retryable"}}`, as on the other routes; the codes are those of
//! `docs/error-codes.md` ("Asset codes").

use assets::{
    AssetError, BlobStore, ImageType, MAX_ASSET_BYTES, Reserve, Usage, check_upload, is_asset_id,
};
use axum::{
    Router,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::StreamExt;
use meldframe_host::{Principal, Refusal};
use meldframe_http::{Failure, Reply, Shared, Slots, bearer, peer};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// What the server decides around the routes.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Uploads being read at once, in all.
    pub uploads: usize,
    /// The same, per principal.
    pub uploads_per_principal: usize,
    /// The longest an upload may take to arrive: an image of 10 MiB needs far longer than a JSON call.
    pub upload_timeout: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            uploads: 8,
            uploads_per_principal: 2,
            upload_timeout: Duration::from_secs(60),
        }
    }
}

/// What a workspace has written besides its assets and the storage limit it is held to: `(bytes of ops, limit)`.
/// The server answers it from its gate.
pub type OpsStorage = Arc<dyn Fn(&str) -> (u64, Option<u64>) + Send + Sync>;

struct State_ {
    shared: Arc<Shared>,
    blobs: Arc<dyn BlobStore>,
    usage: Usage,
    ops_storage: OpsStorage,
    uploads: Slots,
    upload_timeout: Duration,
}

/// The asset routes of one server, with the default options and no storage limit (the assets are counted).
pub fn routes(shared: Arc<Shared>, blobs: Arc<dyn BlobStore>) -> Router {
    let usage = Usage::new(blobs.clone());
    routes_with(
        shared,
        blobs,
        usage,
        Arc::new(|_| (0, None)),
        Options::default(),
    )
}

/// The asset routes of one server. The store is shared by every call: it takes `&self`, so a slow write of one
/// image never makes another call wait. An upload that is not stored yet counts against the storage limit of the
/// workspace together with its ops (`usage` holds the bytes of its assets; give the same `usage` to the gate with
/// `Server::with_extra_storage`), and is refused with `quota_storage` when it does not fit.
pub fn routes_with(
    shared: Arc<Shared>,
    blobs: Arc<dyn BlobStore>,
    usage: Usage,
    ops_storage: OpsStorage,
    options: Options,
) -> Router {
    let state = Arc::new(State_ {
        shared,
        blobs,
        usage,
        ops_storage,
        uploads: Slots::new(options.uploads, options.uploads_per_principal),
        upload_timeout: options.upload_timeout,
    });
    Router::new()
        .route("/v1/assets", post(upload))
        .route("/v1/assets/{id}", get(download))
        .with_state(state)
}

fn failure(e: &AssetError) -> Response {
    let status = match e {
        AssetError::Invalid(_) => StatusCode::BAD_REQUEST,
        AssetError::TooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        AssetError::Unsupported => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        AssetError::NotFound => StatusCode::NOT_FOUND,
        AssetError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        AssetError::Corrupt => StatusCode::UNPROCESSABLE_ENTITY,
    };
    // The text of an unavailable store is for the log, not for the caller.
    if let AssetError::Unavailable(why) = e {
        eprintln!("meldframe-http-assets: unavailable: {why}");
    }
    (
        status,
        axum::Json(
            json!({"error": {"code": e.code(), "message": e.to_string(), "retryable": e.retryable()}}),
        ),
    )
        .into_response()
}

/// Why a store call stopped: the asset is wrong, or the workspace has no room for it.
enum Refused {
    Asset(AssetError),
    Storage(u64),
}

impl State_ {
    /// Store the upload. A new image takes its bytes against the storage limit first, in a reservation that goes
    /// back when the write fails, panics, or finds that the image was there already (another upload wrote it first),
    /// so the same image sent twice at once is charged once. An image the workspace holds costs nothing.
    fn store(&self, workspace: &str, bytes: &[u8]) -> Result<assets::Stored, Refused> {
        let (_, id) = check_upload(bytes).map_err(Refused::Asset)?;
        // Held already (a file that no longer matches counts as held: its bytes were counted and the write below
        // mends it).
        if matches!(
            self.blobs.has(workspace, &id),
            Ok(true) | Err(AssetError::Corrupt)
        ) {
            return self.blobs.put(workspace, bytes).map_err(Refused::Asset);
        }
        let (ops, limit) = (self.ops_storage)(workspace);
        let reservation = self
            .usage
            .reserve(
                workspace,
                bytes.len() as u64,
                ops,
                limit.unwrap_or(u64::MAX),
            )
            .map_err(|e| match e {
                Reserve::Full(limit) => Refused::Storage(limit),
                Reserve::Unknown => {
                    Refused::Asset(AssetError::Unavailable("the asset count is unknown".into()))
                }
            })?;
        let stored = self.blobs.put(workspace, bytes).map_err(Refused::Asset)?;
        if stored.created {
            reservation.keep();
        }
        Ok(stored)
    }

    fn who(&self, req: &Request<Body>) -> Result<Principal, Reply> {
        self.shared
            .client_ok(&peer(req))
            .and_then(|()| self.shared.principal(bearer(req.headers())))
    }
}

async fn upload(State(s): State<Arc<State_>>, req: Request<Body>) -> Response {
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

async fn download(
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
