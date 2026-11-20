//! A batch header as the wire shows it: the `batch` frame of a live stream and a row of the list route (ADR-0021).

use meldframe_activity::Header;
use meldframe_protocol::ServerMsg;
use serde_json::{Value, json};

/// The frame of `h` for the streams of `room`, with the file's count of headers that are not `accepted`.
pub fn frame(room: &str, h: &Header, unreviewed: u32) -> ServerMsg {
    ServerMsg::Batch {
        room: room.to_owned(),
        n: h.n,
        rev: h.rev,
        actor: h.actor.clone(),
        key_prefix: h.key_prefix.clone(),
        tool: h.tool.clone(),
        commands: h.commands,
        written: h.written,
        atomic: h.atomic,
        first_seq: h.first_seq,
        last_seq: h.last_seq,
        at_ms: h.at_ms,
        note: h.note.as_ref().map(|n| n.as_str().to_owned()),
        review: h.review.as_str().to_owned(),
        reviewed_by: h.reviewed_by.clone(),
        reviewed_at_ms: h.reviewed_at_ms,
        unreviewed_count: unreviewed,
    }
}

/// A row of the list route: the fields of the frame without its type, room and count; absent fields are left out.
pub fn row(h: &Header) -> Value {
    let mut v = json!({
        "n": h.n, "rev": h.rev, "actor": h.actor, "keyPrefix": h.key_prefix, "tool": h.tool,
        "commands": h.commands, "written": h.written, "atomic": h.atomic,
        "firstSeq": h.first_seq, "lastSeq": h.last_seq, "atMs": h.at_ms, "review": h.review.as_str(),
    });
    let extra = [
        ("note", h.note.as_ref().map(|n| json!(n.as_str()))),
        ("reviewedBy", h.reviewed_by.as_ref().map(|b| json!(b))),
        ("reviewedAtMs", h.reviewed_at_ms.map(|t| json!(t))),
    ];
    for (k, x) in extra.into_iter().filter_map(|(k, x)| Some((k, x?))) {
        v[k] = x;
    }
    v
}
