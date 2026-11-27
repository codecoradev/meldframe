//! The refusal of an op over REST (audit finding T14, issue 324).

use meldframe_app_model::{Action, Frame};
use serde_json::Value;

/// The code a refusal that names none, or a mistyped one, is given.
const UNREADABLE_CODE: &str = "unavailable";

/// The refusal an answer that is not a 200 stands for, as the frame the server's `rejected` is: the code and the retry flag
/// of `{"error": {"code", "message", "retryable"}}`. The body may come from a proxy or a gateway, so each is taken only if it
/// has the right type: a code that is not a string is `unavailable`, and a flag that is not a boolean means the op may be
/// sent again. Unlike the code of an answer to a read (`error_code`), a code is taken as the server wrote it: the refusal
/// of an op can be an `E_` code of the file's own checks, and the app makes it safe to show where it takes it in
/// (`crate::text`, rule U9). A field no one here reads is ignored. `None` for a 200: the live event confirms the op.
pub(super) fn refused(now_ms: u64, key: String, status: u16, body: &str) -> Option<Action> {
    if status == 200 {
        return None;
    }
    let error = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").cloned());
    let code = error
        .as_ref()
        .and_then(|e| e.get("code")?.as_str().map(String::from))
        .unwrap_or_else(|| UNREADABLE_CODE.into());
    let retryable = error
        .as_ref()
        .and_then(|e| e.get("retryable")?.as_bool())
        .unwrap_or(true);
    Some(Action::Frame {
        now_ms,
        frame: Frame::Rejected {
            key,
            code,
            retryable,
        },
    })
}
