//! The `batch` and `touch` frames of the live socket and the app's use of the raw text, checked in the app layer (audit
//! finding T6, issue 323). A frame whose field is missing, mistyped or out of bound is a hint that is lost (a fetch heals
//! the gap), a field the app does not know is ignored, and a bad answer leaves the state as it was.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, Event, Frame};
use meldframe_app_model::Wire;
use serde_json::{Value, json};

mod common;
use common::Out;
mod batch_wire;
use batch_wire::{bad_headers, header, hint, live, page};

#[test]
fn a_batch_frame_nests_the_header_drops_type_and_room_and_lifts_the_count() {
    let mut f = header(json!({"note": "why"}));
    f["type"] = json!("batch");
    f["room"] = json!("file_dev");
    f["unreviewedCount"] = json!(2);
    let Some(Action::Frame {
        now_ms,
        frame: Frame::Batch {
            header: h,
            unreviewed_count,
        },
    }) = hint(&f.to_string()).0
    else {
        panic!("a batch frame");
    };
    assert_eq!(
        (now_ms, unreviewed_count, h.n, h.note.as_deref()),
        (1000, 2, 3, Some("why"))
    );
}

#[test]
fn a_batch_frame_with_a_bad_field_or_count_is_a_hint_that_is_lost() {
    let frame = |over: Value, count: Value| {
        let mut f = header(over);
        f["type"] = json!("batch");
        f["room"] = json!("file_dev");
        if count != json!("<absent>") {
            f["unreviewedCount"] = count;
        }
        f.to_string()
    };
    for over in bad_headers() {
        assert_eq!(hint(&frame(over.clone(), json!(2))), Out(None), "{over}");
    }
    for count in [
        json!(-1),
        json!(4294967296u64),
        json!("<absent>"),
        json!("2"),
        json!(null),
    ] {
        assert_eq!(hint(&frame(json!({}), count.clone())), Out(None), "{count}");
    }
    for text in [
        "null",
        "\"x\"",
        "[]",
        "{}",
        "<html>",
        "",
        "{\"type\":\"batch\"}",
        "{\"type\":\"nope\"}",
        "{\"type\":7}",
    ] {
        assert_eq!(hint(text), Out(None), "{text}");
    }
}

#[test]
fn a_touch_frame_maps_node_ids_and_needs_a_sequence_number_and_at_most_64_ids() {
    let ids = |n: usize| (0..n).map(|i| format!("n{i}")).collect::<Vec<_>>();
    let touch = |ids: Value, seq: Value| {
        json!({"type": "touch", "actor": "a", "nodeIds": ids, "seq": seq}).to_string()
    };
    let Some(Action::Frame {
        frame: Frame::Touch {
            actor,
            ids: got,
            seq,
        },
        ..
    }) = hint(&touch(json!(["x"]), json!(4))).0
    else {
        panic!("a touch");
    };
    assert_eq!((actor.as_str(), got, seq), ("a", vec!["x".to_string()], 4));
    assert!(
        matches!(hint(&touch(json!(ids(64)), json!(1))).0, Some(Action::Frame { frame: Frame::Touch { ids, .. }, .. }) if ids.len() == 64)
    );
    assert_eq!(hint(&touch(json!(ids(65)), json!(1))), Out(None));
    assert_eq!(hint(&touch(json!(["x"]), json!(-1))), Out(None));
    assert_eq!(hint(&touch(json!(["x", 5]), json!(1))), Out(None));
    assert_eq!(hint(&touch(json!("x"), json!(1))), Out(None));
    assert_eq!(
        hint(&json!({"type": "touch", "actor": "a", "nodeIds": ["x"]}).to_string()),
        Out(None)
    );
    assert_eq!(
        hint(&json!({"type": "touch", "actor": 3, "nodeIds": ["x"], "seq": 1}).to_string()),
        Out(None)
    );
}
#[test]
fn the_app_takes_a_page_a_row_and_a_hint_from_the_raw_text() {
    let mut a = live();
    let body = page(vec![header(json!({}))]).to_string();
    a.dispatch(Action::Wire {
        wire: Wire::Batches { status: 200, body },
    });
    assert_eq!(a.state().activity.unreviewed_count, Some(1));
    assert!(!a.state().activity.lanes.is_empty());
    let mut f = header(json!({"n": 4, "rev": 8, "keyPrefix": "k2"}));
    f["type"] = json!("batch");
    f["unreviewedCount"] = json!(5);
    a.dispatch(Action::Wire {
        wire: Wire::Hint {
            now_ms: 10,
            text: f.to_string(),
        },
    });
    assert_eq!(a.state().activity.unreviewed_count, Some(5));
}

#[test]
fn a_bad_answer_refuses_as_unavailable_and_leaves_the_state_as_it_was() {
    let mut a = live();
    a.dispatch(Action::Wire {
        wire: Wire::Batches {
            status: 200,
            body: page(vec![header(json!({}))]).to_string(),
        },
    });
    a.take_events();
    let before = a.state().clone();
    for status_body in [
        (200, page(vec![header(json!({"n": 0}))]).to_string()),
        (200, "<html>".to_string()),
        (502, "<html>".to_string()),
    ] {
        a.dispatch(Action::Wire {
            wire: Wire::Batches {
                status: status_body.0,
                body: status_body.1.clone(),
            },
        });
        assert_eq!(
            a.take_events(),
            vec![Event::Refused {
                code: "unavailable".into()
            }],
            "{status_body:?}"
        );
        assert_eq!(a.state().activity, before.activity);
    }
    a.dispatch(Action::Wire {
        wire: Wire::Hint {
            now_ms: 11,
            text: "{\"type\":\"batch\",\"n\":0}".into(),
        },
    });
    assert!(
        a.take_events().is_empty(),
        "a hint that does not pass is ignored"
    );
}

#[test]
fn a_touch_from_an_actor_that_is_not_one_is_lost() {
    let touch = |actor: Value| {
        json!({"type": "touch", "actor": actor, "nodeIds": ["x"], "seq": 1}).to_string()
    };
    assert!(matches!(
        hint(&touch(json!("agent:claude"))).0,
        Some(Action::Frame { .. })
    ));
    for actor in [json!(""), json!("a".repeat(129)), json!("agent:\u{202e}x")] {
        assert_eq!(hint(&touch(actor.clone())), Out(None), "{actor}");
    }
}

#[test]
fn a_first_list_that_fails_leaves_the_count_unknown_and_a_bad_frame_changes_nothing() {
    let mut a = live();
    assert_eq!(a.state().activity.unreviewed_count, None);
    a.dispatch(Action::Wire {
        wire: Wire::Batches {
            status: 200,
            body: page(vec![header(json!({"n": 0}))]).to_string(),
        },
    });
    assert_eq!(
        a.state().activity.unreviewed_count,
        None,
        "nothing was read, so nothing is claimed to be zero"
    );
    a.dispatch(Action::Wire {
        wire: Wire::Hint {
            now_ms: 3,
            text: json!({"type": "batch", "n": 0, "unreviewedCount": 3}).to_string(),
        },
    });
    assert_eq!(a.state().activity.unreviewed_count, None);
}
