//! The answer for an asset error: its status, and its code, message and `retryable` as JSON.

use assets::AssetError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub(super) fn failure(e: &AssetError) -> Response {
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
        eprintln!("meldframe-http: assets unavailable: {why}");
    }
    (
        status,
        axum::Json(
            json!({"error": {"code": e.code(), "message": e.to_string(), "retryable": e.retryable()}}),
        ),
    )
        .into_response()
}
