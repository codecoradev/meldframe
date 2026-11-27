//! The answers of the server and the frames of the live socket, checked in the app layer (audit finding T6, issue 323). The
//! shell hands over the status and the text it received (`meldframe_app_model::Wire`); this reads the text with the serde
//! types of the app layer and holds each field to the bound the server itself keeps: the constants of the crate that owns
//! the rule (`meldframe-activity`, `meldframe-checkpoints`, `meldframe-text`), never a copy of them here.
//!
//! "Everything here comes from the network and is trusted for nothing": a body that is not the server's (a proxy's page, a
//! field missing, mistyped or out of bound) is the failure of its request with the code `unavailable`, and the state does
//! not change. A field that no one here knows is ignored, as the server may add one: the answers are tolerant of what they
//! do not read and strict about what they do (the rule is written once, in `docs/architecture.md` section 3d). A frame that
//! does not pass is ignored: it is a hint over the log, and a fetch heals the gap. The checks that follow
//! (`RawHeader::check`, `AssetPage::is_valid`, the label rule of a row) are the app's second line and stay where they are.

mod assets;
mod batches;
mod checkpoints;
mod ops;
mod raster;
mod subscribe;

use meldframe_app_model::{Action, Frame, Wire};
use serde_json::Value;

/// The code of an error answer: a short snake_case word, else `unavailable`. The body may come from a proxy, so it is
/// read as JSON and nothing else of it is kept.
pub(crate) fn error_code(body: &str) -> String {
    let code = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error")?.get("code")?.as_str().map(String::from));
    match code {
        Some(c) if is_code(&c) => c,
        _ => "unavailable".into(),
    }
}

fn is_code(c: &str) -> bool {
    c.len() <= 64
        && c.starts_with(|f: char| f.is_ascii_lowercase())
        && c.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// What a raw answer or frame stands for: the action to carry on with, or `None` for a frame that does not pass.
pub fn action_of(wire: Wire) -> Option<Action> {
    Some(match wire {
        Wire::Batches { status, body } => batches::list(status, &body),
        Wire::BatchReview { status, body } => batches::review(status, &body),
        Wire::Checkpoints { status, body } => checkpoints::list(status, &body),
        Wire::CheckpointPosted { key, status, body } => checkpoints::posted(key, status, &body),
        Wire::CheckpointState { n, status, body } => checkpoints::state(n, status, &body),
        Wire::Assets {
            request,
            after,
            status,
            body,
        } => assets::page(request, after, status, &body),
        Wire::Drawn {
            request,
            status,
            body,
        } => raster::drawn(request, status, &body),
        Wire::Delivered {
            request,
            status,
            body,
        } => raster::delivered(request, status, &body),
        Wire::Hint { now_ms, text } => {
            let frame = hint(&text)?;
            Action::Frame { now_ms, frame }
        }
        Wire::OpRefused {
            now_ms,
            key,
            status,
            body,
        } => ops::refused(now_ms, key, status, &body)?,
        Wire::SubscribeRefused { now_ms, text } => {
            let frame = subscribe::refused(&text)?;
            Action::Frame { now_ms, frame }
        }
    })
}

/// A `batch`, `touch` or `checkpoint` frame of the socket as the app's frame; `None` for any other text and for one that
/// does not pass.
fn hint(text: &str) -> Option<Frame> {
    let Value::Object(map) = serde_json::from_str(text).ok()? else {
        return None;
    };
    match map.get("type")?.as_str()? {
        "batch" => batches::frame(map),
        "touch" => batches::touch(map),
        "checkpoint" => checkpoints::frame(map),
        _ => None,
    }
}
