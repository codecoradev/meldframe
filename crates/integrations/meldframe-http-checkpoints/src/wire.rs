//! A checkpoint as the wire shows it: the `checkpoint` frame of a live stream and a row of the list route (ADR-0023).

use meldframe_checkpoints::Checkpoint;
use meldframe_protocol::ServerMsg;
use serde_json::{Value, json};

/// The frame of `c` for the streams of `room`.
pub fn frame(room: &str, c: &Checkpoint) -> ServerMsg {
    ServerMsg::Checkpoint {
        room: room.to_owned(),
        n: c.n,
        seq: c.seq,
        kind: c.kind.as_str().to_owned(),
        label: c.label.as_ref().map(|l| l.as_str().to_owned()),
        about: c.about.clone(),
        by: c.by.clone(),
        at_ms: c.at_ms,
    }
}

/// A row of the list route and of the answer to a create: the fields of the frame without its type and room; absent
/// fields are left out.
pub fn row(c: &Checkpoint) -> Value {
    let mut v = json!({"n": c.n, "seq": c.seq, "kind": c.kind.as_str(), "atMs": c.at_ms});
    let extra = [
        ("label", c.label.as_ref().map(|l| json!(l.as_str()))),
        ("about", c.about.as_ref().map(|a| json!(a))),
        ("by", c.by.as_ref().map(|b| json!(b))),
    ];
    for (k, x) in extra.into_iter().filter_map(|(k, x)| Some((k, x?))) {
        v[k] = x;
    }
    v
}
