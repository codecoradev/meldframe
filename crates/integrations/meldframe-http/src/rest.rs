//! REST: `POST /v1/files/{file}/tools/{tool}` with `{"ops": [...]}`.

use crate::{Shared, WireOp, reply::Reply};
use axum::{
    body::{Body, to_bytes},
    extract::{ConnectInfo, Path, Request, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    ops: Vec<WireOp>,
}

/// The token of exactly one `Authorization: Bearer <token>` header; the scheme is case-insensitive.
pub(crate) fn bearer(h: &HeaderMap) -> Option<&str> {
    let mut all = h.get_all(header::AUTHORIZATION).iter();
    let (Some(v), None) = (all.next(), all.next()) else {
        return None;
    };
    let (scheme, token) = v.to_str().ok()?.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

pub(crate) fn peer<T>(req: &axum::http::Request<T>) -> String {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or_else(|| "unknown".to_owned(), |c| crate::client_key(c.0.ip()))
}

pub(crate) async fn submit(
    State(s): State<Arc<Shared>>,
    Path((file, tool)): Path<(String, String)>,
    req: Request<Body>,
) -> Response {
    let who = match s
        .client_ok(&peer(&req))
        .and_then(|()| s.principal(bearer(req.headers())))
    {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    let declared = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|n| n > s.max_body) {
        return Reply::too_large(s.max_body).into_response();
    }
    let bytes =
        match tokio::time::timeout(s.body_timeout, to_bytes(req.into_body(), s.max_body)).await {
            Err(_) => return Reply::request_timeout().into_response(),
            Ok(Err(_)) => return Reply::too_large(s.max_body).into_response(),
            Ok(Ok(bytes)) => bytes,
        };
    let Ok(parsed) = serde_json::from_slice::<Payload>(&bytes) else {
        return Reply::bad_request(
            "the body must be JSON of the form {\"ops\": [{\"key\", \"command\"}]}",
        )
        .into_response();
    };
    s.submit(who, tool, file, parsed.ops).await.into_response()
}
