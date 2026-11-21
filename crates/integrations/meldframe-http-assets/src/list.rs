//! `GET /v1/assets`: the assets the workspace of the token holds, a page at a time (E2.5, ADR-0015 follow-up).
//!
//! The answer is `{"assets": [{"id", "bytes", "mime"}], "next": <cursor or null>}`: no content, no name (the store
//! keeps none), no time. The order is the order of the ids (the store keeps no upload time); `after` is the `next` of
//! the page before, `limit` is a whole number above 0 (more than 200 is read as 200) and 50 when left out. It is a read like `GET /v1/assets/{id}`: the token,
//! the peer check and a call slot, and no entry in the call log (the REST asset routes make none).

use super::{State_, failure};
use assets::{AssetError, MAX_LIST, is_asset_id};
use axum::{
    body::Body,
    extract::{Request, State},
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::sync::Arc;

/// The page size when the caller names none.
pub(crate) const DEFAULT_LIMIT: usize = 50;

/// What `limit` and `after` of a query string say. Other keys are ignored. A repeated key counts once, the first.
fn parse(query: Option<&str>) -> Result<(usize, Option<String>), AssetError> {
    let (mut limit, mut after) = (None, None);
    for pair in query.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "limit" if limit.is_none() => limit = Some(value),
            "after" if after.is_none() => after = Some(value),
            _ => {}
        }
    }
    let mistake = || {
        AssetError::Invalid("limit is a whole number above 0; more than 200 is read as 200".into())
    };
    let limit = match limit {
        None => DEFAULT_LIMIT,
        Some(v) if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) => {
            match v.parse::<usize>() {
                Ok(0) => return Err(mistake()),
                // Past the most is the most, and so is a number too long to parse.
                Ok(n) => n.min(MAX_LIST),
                Err(_) => MAX_LIST,
            }
        }
        // Zero, empty, a sign or a word is a mistake of the caller; a larger number is capped, not refused.
        Some(_) => return Err(mistake()),
    };
    let after = match after {
        None => None,
        Some(a) if is_asset_id(a) => Some(a.to_owned()),
        Some(_) => {
            return Err(AssetError::Invalid(
                "after is the next of the page before: an asset id (64 lowercase hex characters)"
                    .into(),
            ));
        }
    };
    Ok((limit, after))
}

pub(crate) async fn list(State(s): State<Arc<State_>>, req: Request<Body>) -> Response {
    let who = match s.who(&req) {
        Ok(who) => who,
        Err(reply) => return reply.into_response(),
    };
    let (limit, after) = match parse(req.uri().query()) {
        Ok(q) => q,
        Err(e) => return failure(&e),
    };
    let (state, ws) = (s.clone(), who.workspace.clone());
    let got = s
        .shared
        .blocking(&who, move || state.blobs.list(&ws, after.as_deref(), limit))
        .await;
    match got {
        Err(reply) => reply.into_response(),
        Ok(Err(e)) => failure(&e),
        Ok(Ok(page)) => {
            let assets: Vec<_> = page
                .items
                .iter()
                .map(|a| json!({"id": a.id, "bytes": a.bytes, "mime": a.kind.mime()}))
                .collect();
            axum::Json(json!({"assets": assets, "next": page.next})).into_response()
        }
    }
}
