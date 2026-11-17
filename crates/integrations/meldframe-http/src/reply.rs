//! What a caller gets back: one JSON shape and one status per outcome, the same on both transports.

use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use meldframe_host::{Accepted, Refusal};
use serde_json::{Value, json};
use std::time::Duration;

/// Why the submitter wrote nothing (or stopped part way).
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// A tool, the gate or the room refused: it has a stable `Refusal::code`.
    Refused(Refusal),
    /// An atomic batch was refused before any op was written: the op at `index` (from 0) would be refused.
    Batch { index: usize, refusal: Refusal },
    /// Not a refusal: the store is down, the registry is full. The text is for the log, not the caller.
    Unavailable(String),
    /// Not a refusal either, but sending the same call again cannot succeed (the data cannot be stored).
    Permanent(String),
}

impl From<Refusal> for Failure {
    fn from(r: Refusal) -> Self {
        Failure::Refused(r)
    }
}

/// A status and a JSON body: `{"accepted": [...]}` or `{"error": {"code", "message", "retryable"}}`.
#[derive(Debug)]
pub struct Reply {
    pub(crate) status: StatusCode,
    pub(crate) body: Value,
    retry_after: Option<Duration>,
}

impl Reply {
    fn error(status: u16, code: &str, message: &str, retryable: bool) -> Self {
        Reply {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            body: json!({"error": {"code": code, "message": message, "retryable": retryable}}),
            retry_after: None,
        }
    }

    pub(crate) fn accepted(done: &[Accepted]) -> Self {
        let list: Vec<_> = done
            .iter()
            .map(|a| json!({"seq": a.seq, "written": a.written}))
            .collect();
        Reply {
            status: StatusCode::OK,
            body: json!({"accepted": list}),
            retry_after: None,
        }
    }

    /// A refusal: its status and code, plus `nodeId` for a conflict and `index` for an op of an atomic batch.
    fn refused(r: &Refusal, index: Option<usize>) -> Self {
        let status = match r {
            Refusal::Forbidden { .. } => 403,
            Refusal::QuotaExceeded { .. } | Refusal::RateExceeded { .. } => 429,
            Refusal::BatchTooLarge { .. } | Refusal::StorageExceeded { .. } => 413,
            Refusal::Conflict { .. } => 409,
            Refusal::Invalid { .. } => 400,
            Refusal::DependencyRejected { .. } | Refusal::Apply(_) => 422,
        };
        let mut reply = Self::error(status, r.code(), &r.to_string(), r.retryable());
        if let Refusal::RateExceeded { retry_after_ms, .. } = r {
            reply.retry_after = Some(Duration::from_millis(*retry_after_ms));
        }
        if let Some(e) = reply.body["error"].as_object_mut() {
            if let Refusal::Conflict { node_id } = r {
                e.insert("nodeId".into(), json!(node_id));
            }
            if let Some(i) = index {
                e.insert("index".into(), json!(i));
            }
        }
        reply
    }

    pub fn failure(f: &Failure) -> Self {
        match f {
            Failure::Refused(r) => Self::refused(r, None),
            Failure::Batch { index, refusal } => Self::refused(refusal, Some(*index)),
            Failure::Permanent(_) => Self::error(
                400,
                "invalid",
                "the store cannot hold this data; change it before sending it again",
                false,
            ),
            Failure::Unavailable(_) => Self::error(
                503,
                "unavailable",
                "the server cannot take this call now; try again later",
                true,
            ),
        }
    }

    pub(crate) fn unauthenticated() -> Self {
        Self::error(
            401,
            "unauthenticated",
            "send a valid token as `Authorization: Bearer <token>`",
            false,
        )
    }

    pub(crate) fn rate_limited(wait: Duration) -> Self {
        let mut r = Self::error(
            429,
            "rate_limited",
            "too many calls; wait and send again",
            true,
        );
        r.retry_after = Some(wait);
        r
    }

    pub(crate) fn too_large(max: usize) -> Self {
        Self::error(
            413,
            "too_large",
            &format!("the request is larger than {max} bytes"),
            false,
        )
    }

    pub fn bad_request(why: &str) -> Self {
        Self::error(400, "bad_request", why, false)
    }

    pub(crate) fn request_timeout() -> Self {
        Self::error(
            408,
            "request_timeout",
            "the request body took too long to arrive",
            true,
        )
    }

    pub(crate) fn timeout() -> Self {
        Self::error(
            504,
            "timeout",
            "the call took too long; its ops may still be written, send it again with the same keys",
            true,
        )
    }

    pub(crate) fn internal() -> Self {
        Self::error(500, "internal", "the call failed inside the server", true)
    }
}

impl IntoResponse for Reply {
    fn into_response(self) -> Response {
        let mut res = (self.status, axum::Json(self.body)).into_response();
        let h = res.headers_mut();
        if let Some(w) = self.retry_after {
            let secs = w.as_secs() + u64::from(w.subsec_nanos() > 0);
            if let Ok(v) = HeaderValue::from_str(&secs.max(1).to_string()) {
                h.insert(header::RETRY_AFTER, v);
            }
        }
        if self.status == StatusCode::UNAUTHORIZED {
            h.insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        res
    }
}
