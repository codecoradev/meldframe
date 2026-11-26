//! The checkpoint routes: the list, a post, the file at a checkpoint, and the `checkpoint` frame.

use super::error_code;
use meldframe_app_model::{Action, CheckpointRequest, CheckpointRow, Frame, MAX_SAFE_INT};
use meldframe_checkpoints::{MAX_LABEL_CHARS, MAX_PER_FILE};
use meldframe_protocol::Node;
use meldframe_text::actor;
use serde::Deserialize;
use serde_json::{Map, Value};

/// A file at a checkpoint holds the nodes of one file: far more than a file has, so only a runaway answer is refused.
const MAX_NODES: usize = 200_000;

fn failed(request: CheckpointRequest, code: &str) -> Action {
    Action::CheckpointRequestFailed {
        request,
        code: code.into(),
    }
}

/// Whether a row that has the right shape is held to the bounds of the server: a number, the times a front end holds, a
/// label of at most `MAX_LABEL_CHARS` characters and actors that are actors. The app checks the label's hidden-text rule
/// again (`Label::parse`) before a row is made from it.
fn in_bounds(r: &CheckpointRow) -> bool {
    r.n >= 1
        && r.seq <= MAX_SAFE_INT
        && r.at_ms <= MAX_SAFE_INT
        && r.label
            .as_deref()
            .is_none_or(|l| l.chars().count() <= MAX_LABEL_CHARS)
        && r.about.as_deref().is_none_or(|a| actor(a).is_ok())
        && r.by.as_deref().is_none_or(|b| actor(b).is_ok())
}

/// A row of the list, of a create or of a frame: `None` when a field is missing, mistyped or out of bound; a field it does
/// not know is ignored.
fn row(v: Value) -> Option<CheckpointRow> {
    serde_json::from_value::<CheckpointRow>(v)
        .ok()
        .filter(in_bounds)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rows {
    checkpoints: Vec<CheckpointRow>,
    can_create: bool,
}

/// The answer of `GET .../checkpoints`. A 404 with no error body is a server that has no such route.
pub(super) fn list(status: u16, body: &str) -> Action {
    let list = CheckpointRequest::List {};
    if status == 404
        && serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|v| v.get("error").cloned())
            .is_none()
    {
        return failed(list, "not_found");
    }
    if status != 200 {
        return failed(list, &error_code(body));
    }
    match serde_json::from_str::<Rows>(body) {
        Ok(r) if r.checkpoints.len() <= MAX_PER_FILE && r.checkpoints.iter().all(in_bounds) => {
            Action::CheckpointsLoaded {
                checkpoints: r.checkpoints,
                can_create: r.can_create,
            }
        }
        _ => failed(list, "unavailable"),
    }
}

#[derive(Deserialize)]
struct Posted {
    checkpoint: CheckpointRow,
}

/// The answer of `POST .../checkpoints`, `{checkpoint, created}`, with the key the post carried. The app needs only the
/// row: whether it was made or already there is not read.
pub(super) fn posted(key: String, status: u16, body: &str) -> Action {
    let request = CheckpointRequest::Post { key: key.clone() };
    if status != 200 {
        let code = error_code(body);
        return failed(request, &code);
    }
    match serde_json::from_str::<Posted>(body) {
        Ok(p) if in_bounds(&p.checkpoint) => Action::CheckpointPosted {
            key,
            checkpoint: p.checkpoint,
        },
        _ => failed(request, "unavailable"),
    }
}

#[derive(Deserialize)]
struct State {
    seq: u64,
    nodes: Vec<Node>,
}

/// The answer of `GET .../checkpoints/{n}/state`, `{seq, nodes}`, with the number it was asked for. A node is the protocol's
/// own closed type, which refuses a field it does not know: a state that holds one is refused whole.
pub(super) fn state(n: u32, status: u16, body: &str) -> Action {
    let request = CheckpointRequest::State { n };
    if status != 200 {
        return failed(request, &error_code(body));
    }
    match serde_json::from_str::<State>(body) {
        Ok(s) if s.seq <= MAX_SAFE_INT && s.nodes.len() <= MAX_NODES => {
            Action::CheckpointStateLoaded {
                n,
                seq: s.seq,
                nodes: s.nodes,
            }
        }
        _ => failed(request, "unavailable"),
    }
}

/// The flat `checkpoint` frame of the socket as `Frame::Checkpoint`: the row nested, `type` and `room` dropped.
pub(super) fn frame(mut map: Map<String, Value>) -> Option<Frame> {
    map.remove("type");
    map.remove("room");
    Some(Frame::Checkpoint {
        checkpoint: row(Value::Object(map))?,
    })
}
