//! The batch routes: the list, the review, and the `batch` and `touch` frames.

use super::error_code;
use meldframe_activity::{MAX_COUNT, MAX_NOTE_CHARS, MAX_PAGE, MAX_RING_IDS, RawHeader};
use meldframe_app_model::{Action, BatchHeader, BatchPage, Frame, MAX_SAFE_INT};
use meldframe_text::actor;
use serde_json::{Map, Value};

fn failed(code: &str) -> Action {
    Action::BatchRequestFailed { code: code.into() }
}

/// Whether a header that has the right shape is held to the bounds of the server. The rule of a number, an actor, a key
/// prefix and a tool is `RawHeader::check`, the one the server's store and tools apply and the reducer runs again on every
/// header it takes in; the rest is what the reducer cleans and a front end holds exactly: the note is at most
/// `MAX_NOTE_CHARS` characters, the times are whole numbers a front end holds, and a reviewer is an actor.
fn in_bounds(h: &BatchHeader) -> bool {
    RawHeader::from(h.clone()).check().is_some()
        && [h.first_seq, h.last_seq, h.at_ms]
            .iter()
            .all(|v| *v <= MAX_SAFE_INT)
        && h.note
            .as_deref()
            .is_none_or(|n| n.chars().count() <= MAX_NOTE_CHARS)
        && h.reviewed_by.as_deref().is_none_or(|w| actor(w).is_ok())
        && h.reviewed_at_ms.is_none_or(|v| v <= MAX_SAFE_INT)
}

/// A header from a frame or a row of the list: `None` when a field is missing, mistyped or out of bound; a field it does
/// not know is ignored.
fn header(v: Value) -> Option<BatchHeader> {
    serde_json::from_value::<BatchHeader>(v)
        .ok()
        .filter(in_bounds)
}

/// The answer of `GET /v1/files/{file}/batches`.
pub(super) fn list(status: u16, body: &str) -> Action {
    if status != 200 {
        return failed(&error_code(body));
    }
    let page = serde_json::from_str::<BatchPage>(body).ok().filter(|p| {
        p.batches.len() <= MAX_PAGE
            && p.unreviewed_count <= MAX_COUNT
            && p.batches.iter().all(in_bounds)
    });
    page.map_or_else(
        || failed("unavailable"),
        |page| Action::BatchesLoaded { page },
    )
}

/// The answer of `PUT .../review`, `{batch, unreviewedCount}`: the row alone (the frame carries the count). The count has
/// to be there and be one the server could send, though the app takes only the row.
pub(super) fn review(status: u16, body: &str) -> Action {
    if status != 200 {
        return failed(&error_code(body));
    }
    let row = serde_json::from_str::<Value>(body).ok().and_then(|mut v| {
        let count_ok = v
            .get("unreviewedCount")?
            .as_u64()
            .is_some_and(|c| c <= u64::from(MAX_COUNT));
        let batch = v.get_mut("batch")?.take();
        count_ok.then_some(batch)
    });
    row.and_then(header).map_or_else(
        || failed("unavailable"),
        |batch| Action::BatchReviewed { batch },
    )
}

/// The flat `batch` frame of the socket as `Frame::Batch`: the header nested, `type` and `room` dropped.
pub(super) fn frame(mut map: Map<String, Value>) -> Option<Frame> {
    map.remove("type");
    map.remove("room");
    let count = map.remove("unreviewedCount")?.as_u64()?;
    Some(Frame::Batch {
        header: header(Value::Object(map))?,
        unreviewed_count: u32::try_from(count).ok().filter(|c| *c <= MAX_COUNT)?,
    })
}

/// The `touch` frame of the socket as `Frame::Touch` (`nodeIds` is `ids` there); `None` without a sequence number, with an
/// actor that is not one, or with more ids than the server sends.
pub(super) fn touch(map: Map<String, Value>) -> Option<Frame> {
    let who = map.get("actor")?.as_str()?;
    actor(who).ok()?;
    let ids = map.get("nodeIds")?.as_array()?;
    if ids.len() > MAX_RING_IDS {
        return None;
    }
    let ids = ids
        .iter()
        .map(|i| i.as_str().map(String::from))
        .collect::<Option<Vec<_>>>()?;
    let seq = map.get("seq")?.as_u64().filter(|s| *s <= MAX_SAFE_INT)?;
    Some(Frame::Touch {
        actor: who.to_string(),
        ids,
        seq,
    })
}
