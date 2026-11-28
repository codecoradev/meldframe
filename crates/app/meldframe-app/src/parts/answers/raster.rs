//! What the shell reports of a drawing or an upload (`Wire::Drawn`, `Wire::Delivered`) and the `captureRequest` frame
//! (`Wire::Capture`), checked once, here (finding T13 of the thin UI audit, decision 43). The bounds are the server's: the
//! id and the token of a request go into a path and a header, so they are plain characters of a length the server keeps;
//! the numbers a front end holds exactly (2^53 - 1, `MAX_SAFE_INT`).

use super::error_code;
use meldframe_app_model::{Action, CaptureAsk, MAX_SAFE_INT, PngOp};
use meldframe_text::MAX_NODE_REF;
use serde::Deserialize;
use serde_json::Value;

/// The longest request id or upload token read from a request, in bytes. The server's ids are 20 bytes and its tokens 32.
const MAX_ID: usize = 128;

/// The request as the server sends it. A field the app does not know is ignored (the server may add one).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    request_id: String,
    #[serde(default)]
    node_id: Option<String>,
    scale: f64,
    max_pixels: u64,
    min_seq: u64,
    upload_token: String,
    timeout_ms: u64,
}

/// A request id or an upload token: ASCII letters, digits, `_` and `-`, not empty, within [`MAX_ID`] bytes.
fn is_safe(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ID
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Whether a request is one the app takes: the id and the token go into a path and a header, so they are plain characters of a
/// length the server keeps; the node id is at most `MAX_NODE_REF` bytes; the scale is a number and the others are whole numbers
/// within `MAX_SAFE_INT`. The wire path asks it of every frame, and the app asks it again of a request that was dispatched
/// as an action.
pub fn ask_is_safe(ask: &CaptureAsk) -> bool {
    is_safe(&ask.request_id)
        && is_safe(ask.upload_token.expose())
        && ask.node_id.as_ref().is_none_or(|n| n.len() <= MAX_NODE_REF)
        && ask.scale.is_finite()
        && [ask.max_pixels, ask.min_seq, ask.timeout_ms]
            .iter()
            .all(|n| *n <= MAX_SAFE_INT)
}

/// The action a `captureRequest` frame stands for; `None` for any other text, and for one that does not pass.
pub(super) fn capture(now_ms: u64, text: &str) -> Option<Action> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v.get("type")?.as_str()? != "captureRequest" {
        return None;
    }
    let r: Raw = serde_json::from_value(v).ok()?;
    let ask = CaptureAsk {
        request_id: r.request_id,
        node_id: r.node_id,
        scale: r.scale,
        max_pixels: r.max_pixels,
        min_seq: r.min_seq,
        upload_token: r.upload_token.into(),
        timeout_ms: r.timeout_ms,
    };
    ask_is_safe(&ask).then_some(Action::Png {
        op: PngOp::Capture { now_ms, ask },
    })
}

/// The report of a drawing for `request`: the size the shell drew, or the failure it stands for. A body that is not
/// `{"width", "height", "bytes"}` of whole numbers (the sides within 32 bits, the bytes within `MAX_SAFE_INT`) is a
/// failed drawing, `render_failed`.
pub(super) fn drawn(request: u32, status: u16, body: &str) -> Action {
    #[derive(Deserialize)]
    struct Size {
        width: u32,
        height: u32,
        bytes: u64,
    }
    let failed = |code: String| PngOp::DrawFailed { request, code };
    let op = if status != 200 {
        // A body that names no code is a drawing that failed, not a server that is away.
        match error_code(body).as_str() {
            "unavailable" => failed("render_failed".into()),
            code => failed(code.into()),
        }
    } else {
        match serde_json::from_str::<Size>(body) {
            Ok(s) if s.bytes <= MAX_SAFE_INT => PngOp::Drawn {
                request,
                width: s.width,
                height: s.height,
                bytes: s.bytes,
            },
            _ => failed("render_failed".into()),
        }
    };
    Action::Png { op }
}

/// The answer of the sink of `request`: a 200 is delivered, anything else the failure its body names (`unavailable` for a
/// body that is not the server's).
pub(super) fn delivered(request: u32, status: u16, body: &str) -> Action {
    Action::Png {
        op: if status == 200 {
            PngOp::Delivered { request }
        } else {
            PngOp::DeliveryFailed {
                request,
                code: error_code(body),
            }
        },
    }
}
