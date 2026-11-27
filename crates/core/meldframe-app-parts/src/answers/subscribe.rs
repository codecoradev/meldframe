//! The `error` frame of the live socket that refuses a `subscribe` (audit finding T14, issue 324).

use super::is_code;
use meldframe_app_model::{Frame, MAX_SAFE_INT};
use serde_json::Value;

/// The refusal an `error` frame stands for; `None` for any other text. The shell hands over only the frame that answers
/// the last subscribe it sent, so the app has no id to match. A frame whose code is missing or is not a code, or whose
/// `retryAfterMs` is present and is not a whole number a front end holds, is a refusal the app cannot read: it carries no
/// code, and the rule gives the connection up (the fail-safe: it reconnects from the last confirmed number). A field that
/// no one here reads (`message`, `retryable`, `id`) is ignored.
pub(super) fn refused(text: &str) -> Option<Frame> {
    let Value::Object(map) = serde_json::from_str(text).ok()? else {
        return None;
    };
    if map.get("type")?.as_str()? != "error" {
        return None;
    }
    let code = map
        .get("code")
        .and_then(Value::as_str)
        .filter(|c| is_code(c))
        .map(String::from);
    let wait = match map.get("retryAfterMs") {
        None | Some(Value::Null) => Some(None),
        Some(v) => v.as_u64().filter(|ms| *ms <= MAX_SAFE_INT).map(Some),
    };
    Some(match wait {
        Some(retry_after_ms) => Frame::SubscribeRefused {
            code,
            retry_after_ms,
        },
        None => Frame::SubscribeRefused {
            code: None,
            retry_after_ms: None,
        },
    })
}
