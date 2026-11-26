//! The `checkpoint` frame of the socket, checked in the app layer (audit finding T6, issue 323). The table is that of
//! `checkpoints.test.ts` of `web-transport`, which moved here, with the bounds of the server itself: a frame whose field is
//! missing, mistyped or out of bound is a hint that is lost (a read heals the gap), and a field the app does not know is
//! ignored.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, Frame};
use meldframe_app_model::Wire;
use meldframe_app_parts::answers::action_of;
use serde_json::{Value, json};

mod common;
use common::Out;

fn row(over: Value) -> Value {
    let mut r =
        json!({"n": 3, "seq": 12, "kind": "manual", "label": "v1", "by": "user:bea", "atMs": 5});
    for (k, v) in over.as_object().cloned().unwrap_or_default() {
        if v == json!("<absent>") {
            r.as_object_mut().unwrap().remove(&k);
        } else {
            r[k] = v;
        }
    }
    r
}

fn hint(text: &str) -> Out {
    Out(action_of(Wire::Hint {
        now_ms: 1000,
        text: text.into(),
    }))
}

fn bad_rows() -> Vec<Value> {
    let long = |c: &str, n: usize| json!(c.repeat(n));
    vec![
        json!({"n": 0}),
        json!({"n": 1.5}),
        json!({"n": 4294967296u64}),
        json!({"n": "3"}),
        json!({"seq": -1}),
        json!({"seq": 9007199254740992u64}),
        json!({"kind": "snapshot"}),
        json!({"kind": 5}),
        json!({"atMs": -1}),
        json!({"atMs": "<absent>"}),
        json!({"label": 5}),
        // A label is 128 characters at most (meldframe_checkpoints::MAX_LABEL_CHARS); an actor 128 bytes.
        json!({"label": long("x", 129)}),
        json!({"about": ""}),
        json!({"about": long("a", 129)}),
        json!({"about": long("é", 65)}),
        json!({"by": 1}),
        json!({"by": long("b", 129)}),
        json!({"by": "user:\u{202e}x"}),
        json!({"n": null}),
        json!({"seq": null}),
    ]
}

#[test]
fn a_checkpoint_frame_nests_the_row_and_drops_type_and_room() {
    let mut f = row(json!({}));
    f["type"] = json!("checkpoint");
    f["room"] = json!("file_dev");
    let Some(Action::Frame {
        frame: Frame::Checkpoint { checkpoint },
        ..
    }) = hint(&f.to_string()).0
    else {
        panic!("a checkpoint frame");
    };
    assert_eq!(
        (
            checkpoint.n,
            checkpoint.label.as_deref(),
            checkpoint.by.as_deref()
        ),
        (3, Some("v1"), Some("user:bea"))
    );
}

#[test]
fn a_checkpoint_frame_with_a_bad_field_is_a_hint_that_is_lost() {
    for over in bad_rows() {
        let mut f = row(over.clone());
        f["type"] = json!("checkpoint");
        f["room"] = json!("file_dev");
        assert_eq!(hint(&f.to_string()), Out(None), "{over}");
    }
    for text in ["null", "\"x\"", "[]", "<html>", "{\"type\":\"checkpoint\"}"] {
        assert_eq!(hint(text), Out(None), "{text}");
    }
}

#[test]
fn a_field_the_app_does_not_know_is_ignored_in_a_frame_and_the_edges_of_a_row_are_taken() {
    let mut f = row(json!({"tags": ["new"], "more": 1}));
    f["type"] = json!("checkpoint");
    f["room"] = json!("file_dev");
    assert!(matches!(hint(&f.to_string()).0, Some(Action::Frame { .. })));
    let mut edge = row(
        json!({"n": 4294967295u64, "seq": 9007199254740991u64, "atMs": 9007199254740991u64,
        "label": "é".repeat(128), "about": "a".repeat(128), "by": "b".repeat(128)}),
    );
    edge["type"] = json!("checkpoint");
    assert!(matches!(
        hint(&edge.to_string()).0,
        Some(Action::Frame { .. })
    ));
}
