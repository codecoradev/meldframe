//! The answers of the checkpoint routes and the `checkpoint` frame, checked in the app layer (audit finding T6, issue 323).
//! The tables are those of `checkpoints.test.ts` of `web-transport`, which moved here: a body whose field is missing,
//! mistyped or out of bound is the failure of its request (`unavailable`, naming the request) and changes nothing.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, CheckpointRequest, CheckpointsStatus, Event, Frame};
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

fn rows(status: u16, body: &str) -> Out {
    Out(action_of(Wire::Checkpoints {
        status,
        body: body.into(),
    }))
}

fn posted(status: u16, body: &str) -> Out {
    Out(action_of(Wire::CheckpointPosted {
        key: "k1".into(),
        status,
        body: body.into(),
    }))
}

fn state(status: u16, body: &str) -> Out {
    Out(action_of(Wire::CheckpointState {
        n: 3,
        status,
        body: body.into(),
    }))
}

fn hint(text: &str) -> Out {
    Out(action_of(Wire::Hint {
        now_ms: 1000,
        text: text.into(),
    }))
}

fn failed(request: CheckpointRequest, code: &str) -> Out {
    Out(Some(Action::CheckpointRequestFailed {
        request,
        code: code.into(),
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
fn the_list_is_checkpoints_loaded_with_can_create() {
    let run = row(
        json!({"n": 4, "kind": "runStart", "label": "<absent>", "by": "<absent>", "about": "agent:x", "atMs": 6}),
    );
    let body = json!({"checkpoints": [run, row(json!({}))], "canCreate": true}).to_string();
    let Some(Action::CheckpointsLoaded {
        checkpoints,
        can_create,
    }) = rows(200, &body).0
    else {
        panic!("a list");
    };
    assert!(can_create);
    assert_eq!(checkpoints.iter().map(|c| c.n).collect::<Vec<_>>(), [4, 3]);
    assert_eq!(checkpoints[0].about.as_deref(), Some("agent:x"));
    assert!(matches!(
        rows(
            200,
            &json!({"checkpoints": [], "canCreate": false}).to_string()
        )
        .0,
        Some(Action::CheckpointsLoaded { .. })
    ));
}

#[test]
fn a_list_that_is_not_one_is_a_failure_of_the_read() {
    let list = || failed(CheckpointRequest::List {}, "unavailable");
    for over in bad_rows() {
        let body = json!({"checkpoints": [row(over.clone())], "canCreate": true}).to_string();
        assert_eq!(rows(200, &body), list(), "{over}");
    }
    for body in [
        json!({"checkpoints": [row(json!({}))]}).to_string(),
        json!({"checkpoints": (0..101).map(|_| row(json!({}))).collect::<Vec<_>>(), "canCreate": true}).to_string(),
        json!({"checkpoints": [], "canCreate": 1}).to_string(),
        json!({"checkpoints": null, "canCreate": true}).to_string(),
        json!({"checkpoints": [], "canCreate": null}).to_string(),
        "null".into(), "<html>".into(), "".into(),
    ] {
        assert_eq!(rows(200, &body), list(), "{body}");
    }
    let hundred = json!({"checkpoints": (0..100).map(|_| row(json!({}))).collect::<Vec<_>>(), "canCreate": true}).to_string();
    assert!(matches!(
        rows(200, &hundred).0,
        Some(Action::CheckpointsLoaded { .. })
    ));
}

#[test]
fn a_server_without_the_module_answers_404_with_no_error_body_and_that_is_not_found() {
    let list = |code: &str| failed(CheckpointRequest::List {}, code);
    assert_eq!(rows(404, "<html>"), list("not_found"));
    assert_eq!(rows(404, "{}"), list("not_found"));
    assert_eq!(
        rows(404, &json!({"error": {"code": "not_found"}}).to_string()),
        list("not_found")
    );
    assert_eq!(
        rows(404, &json!({"error": {"code": "no_such_file"}}).to_string()),
        list("no_such_file")
    );
    assert_eq!(
        rows(
            403,
            &json!({"error": {"code": "forbidden", "message": "m"}}).to_string()
        ),
        list("forbidden")
    );
    assert_eq!(
        rows(
            500,
            &json!({"error": {"code": "x".repeat(5000)}}).to_string()
        ),
        list("unavailable")
    );
}

#[test]
fn a_post_answers_with_its_row_and_the_key_it_was_sent_with() {
    let body = json!({"checkpoint": row(json!({})), "created": true}).to_string();
    let Some(Action::CheckpointPosted { key, checkpoint }) = posted(200, &body).0 else {
        panic!("a row");
    };
    assert_eq!((key.as_str(), checkpoint.n), ("k1", 3));
    let post = |code: &str| failed(CheckpointRequest::Post { key: "k1".into() }, code);
    assert_eq!(
        posted(200, &json!({"created": true}).to_string()),
        post("unavailable")
    );
    assert_eq!(
        posted(
            200,
            &json!({"checkpoint": row(json!({"n": 0}))}).to_string()
        ),
        post("unavailable")
    );
    assert_eq!(posted(200, "<html>"), post("unavailable"));
    for code in ["checkpoint_full", "key_reused", "invalid", "forbidden"] {
        let body = json!({"error": {"code": code, "message": "m", "n": 2}}).to_string();
        assert_eq!(posted(409, &body), post(code));
    }
}

#[test]
fn the_state_is_checkpoint_state_loaded_with_the_number_it_was_asked_for() {
    let node = json!({"id": "a", "type": "rect"});
    let Some(Action::CheckpointStateLoaded { n, seq, nodes }) =
        state(200, &json!({"seq": 12, "nodes": [node]}).to_string()).0
    else {
        panic!("a state");
    };
    assert_eq!((n, seq, nodes.len(), nodes[0].id.as_str()), (3, 12, 1, "a"));
    let bad = |body: Value| {
        assert_eq!(
            state(200, &body.to_string()),
            failed(CheckpointRequest::State { n: 3 }, "unavailable"),
            "{body}"
        )
    };
    for body in [
        json!({"seq": -1, "nodes": []}),
        json!({"seq": 1}),
        json!({"seq": 1, "nodes": [1]}),
        json!({"seq": 1, "nodes": "x"}),
        json!(null),
        json!({"seq": 9007199254740992u64, "nodes": []}),
        json!({"seq": 1, "nodes": [{"id": "a", "type": "nope"}]}),
        json!({"seq": 1, "nodes": null}),
        json!({"seq": null, "nodes": []}),
        // A node is the protocol's own closed type (it refuses a field it does not know), so an answer that holds one is
        // refused whole: tolerance is for the answers of the routes, not for the file's own nodes.
        json!({"seq": 1, "nodes": [{"id": "a", "type": "rect", "extra": 1}]}),
    ] {
        bad(body);
    }
    assert_eq!(
        state(
            422,
            &json!({"error": {"code": "too_far", "message": "m"}}).to_string()
        ),
        failed(CheckpointRequest::State { n: 3 }, "too_far")
    );
    assert_eq!(
        state(200, "<html>"),
        failed(CheckpointRequest::State { n: 3 }, "unavailable")
    );
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
fn a_field_the_app_does_not_know_is_ignored_in_every_body_a_row_and_a_frame() {
    let mut r = row(json!({}));
    r["tags"] = json!(["new"]);
    let list = json!({"checkpoints": [r.clone()], "canCreate": true, "extra": 1}).to_string();
    assert!(matches!(
        rows(200, &list).0,
        Some(Action::CheckpointsLoaded { .. })
    ));
    let post = json!({"checkpoint": r.clone(), "created": true, "more": 2}).to_string();
    assert!(matches!(
        posted(200, &post).0,
        Some(Action::CheckpointPosted { .. })
    ));
    let state_body = json!({"seq": 1, "nodes": [], "at": "x"}).to_string();
    assert!(matches!(
        state(200, &state_body).0,
        Some(Action::CheckpointStateLoaded { .. })
    ));
    let mut f = r;
    f["type"] = json!("checkpoint");
    f["room"] = json!("file_dev");
    assert!(matches!(hint(&f.to_string()).0, Some(Action::Frame { .. })));
}

#[test]
fn the_edges_of_a_row_are_taken_and_a_state_with_too_many_nodes_is_refused() {
    let edge = row(
        json!({"n": 4294967295u64, "seq": 9007199254740991u64, "atMs": 9007199254740991u64,
        "label": "é".repeat(128), "about": "a".repeat(128), "by": "b".repeat(128)}),
    );
    let body = json!({"checkpoints": [edge], "canCreate": true}).to_string();
    assert!(matches!(
        rows(200, &body).0,
        Some(Action::CheckpointsLoaded { .. })
    ));
    let nodes = |n: usize| {
        let one = r#"{"id":"a","type":"rect"}"#;
        format!(r#"{{"seq":1,"nodes":[{}]}}"#, vec![one; n].join(","))
    };
    assert!(matches!(
        state(200, &nodes(200_000)).0,
        Some(Action::CheckpointStateLoaded { .. })
    ));
    assert_eq!(
        state(200, &nodes(200_001)),
        failed(CheckpointRequest::State { n: 3 }, "unavailable")
    );
}

fn live() -> App {
    let mut a = App::new("user:me", "c0", 1, Vec::new(), 0).unwrap();
    a.dispatch(Action::Connected { now_ms: 0 });
    a.take_events();
    a
}

#[test]
fn the_app_takes_the_list_from_the_raw_text_and_keeps_its_state_when_the_text_is_bad() {
    let mut a = live();
    let body = json!({"checkpoints": [row(json!({}))], "canCreate": true}).to_string();
    a.dispatch(Action::Wire {
        wire: Wire::Checkpoints { status: 200, body },
    });
    assert_eq!(
        (a.state().checkpoints.len(), a.state().can_create_checkpoint),
        (1, true)
    );
    assert_eq!(a.state().checkpoints_status, CheckpointsStatus::Loaded);
    a.take_events();
    let before = a.state().clone();
    a.dispatch(Action::Wire {
        wire: Wire::Checkpoints {
            status: 200,
            body: json!({"checkpoints": [row(json!({"n": 0}))], "canCreate": false}).to_string(),
        },
    });
    assert_eq!(
        a.take_events(),
        vec![Event::Refused {
            code: "unavailable".into()
        }]
    );
    assert_eq!(a.state().checkpoints, before.checkpoints);
    assert_eq!(
        a.state().can_create_checkpoint,
        before.can_create_checkpoint
    );
    // A server with no such route says so, and the app reads that as none kept.
    a.dispatch(Action::Wire {
        wire: Wire::Checkpoints {
            status: 404,
            body: "<html>".into(),
        },
    });
    assert_eq!(a.state().checkpoints_status, CheckpointsStatus::Absent);
}
