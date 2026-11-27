//! What the shell reports of a drawing or a delivery (`Wire::Drawn`, `Wire::Delivered`), checked once, here (finding T13 of
//! the thin UI audit, decision 43): the numbers are within what a front end holds exactly (2^53 - 1, `MAX_SAFE_INT`), and a
//! body that is not what the shell sends is a failure with a code of its own.

use super::error_code;
use meldframe_app_model::{Action, MAX_SAFE_INT, PngOp};
use serde::Deserialize;

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
