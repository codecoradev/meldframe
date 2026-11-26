//! The `checkpoint` frame of the socket and the row it carries.

use meldframe_app_model::{CheckpointRow, Frame, MAX_SAFE_INT};
use meldframe_checkpoints::MAX_LABEL_CHARS;
use meldframe_text::actor;
use serde_json::{Map, Value};

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

/// A row of a frame: `None` when a field is missing, mistyped or out of bound; a field it does not know is ignored.
fn row(v: Value) -> Option<CheckpointRow> {
    serde_json::from_value::<CheckpointRow>(v)
        .ok()
        .filter(in_bounds)
}

/// The flat `checkpoint` frame of the socket as `Frame::Checkpoint`: the row nested, `type` and `room` dropped.
pub(super) fn frame(mut map: Map<String, Value>) -> Option<Frame> {
    map.remove("type");
    map.remove("room");
    Some(Frame::Checkpoint {
        checkpoint: row(Value::Object(map))?,
    })
}
