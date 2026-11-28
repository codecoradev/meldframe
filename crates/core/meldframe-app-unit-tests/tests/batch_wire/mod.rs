//! The headers, pages and answers of the batch tests (`answers_batches.rs`, `answers_hints.rs`).
#![allow(dead_code)]

use crate::common::Out;
use meldframe_app::parts::answers::action_of;
use meldframe_app::{Action, App};
use meldframe_app_model::Wire;
use serde_json::{Value, json};

pub fn header(over: Value) -> Value {
    let mut h = json!({
        "n": 3, "rev": 7, "actor": "agent:claude", "keyPrefix": "k1", "tool": "apply_commands", "commands": 4,
        "written": 4, "atomic": true, "firstSeq": 10, "lastSeq": 13, "atMs": 5000, "review": "unreviewed"
    });
    for (k, v) in over.as_object().cloned().unwrap_or_default() {
        if v == json!("<absent>") {
            h.as_object_mut().unwrap().remove(&k);
        } else {
            h[k] = v;
        }
    }
    h
}

pub fn page(batches: Vec<Value>) -> Value {
    json!({"batches": batches, "hasMore": true, "unreviewedCount": 1, "canReview": false})
}

pub fn list(status: u16, body: &str) -> Out {
    Out(action_of(Wire::Batches {
        status,
        body: body.into(),
    }))
}

pub fn review(status: u16, body: &str) -> Out {
    Out(action_of(Wire::BatchReview {
        status,
        body: body.into(),
    }))
}

pub fn hint(text: &str) -> Out {
    Out(action_of(Wire::Hint {
        now_ms: 1000,
        text: text.into(),
    }))
}

pub fn failure(code: &str) -> Out {
    Out(Some(Action::BatchRequestFailed { code: code.into() }))
}

/// What a bad field is: replaced, or absent.
pub fn bad_headers() -> Vec<Value> {
    let long = |c: &str, n: usize| json!(c.repeat(n));
    vec![
        json!({"n": 0}),
        json!({"n": 1.5}),
        json!({"n": 4294967296u64}),
        json!({"n": "3"}),
        json!({"rev": -1}),
        // One above the server's own ceiling of a revision (meldframe_activity::MAX_REV).
        json!({"rev": 281474976710657u64}),
        json!({"rev": "7"}),
        json!({"actor": ""}),
        json!({"actor": 5}),
        // The bounds of the server are in bytes: 129 of them is too long, and 65 two-byte characters are 130.
        json!({"actor": long("a", 129)}),
        json!({"actor": long("é", 65)}),
        json!({"actor": "agent:\u{202e}x"}),
        json!({"keyPrefix": long("k", 101)}),
        json!({"keyPrefix": ""}),
        json!({"tool": 3}),
        json!({"tool": long("t", 65)}),
        json!({"tool": long("é", 33)}),
        json!({"commands": -1}),
        json!({"written": 10000000000u64}),
        json!({"atomic": "yes"}),
        json!({"firstSeq": 9007199254740992u64}),
        json!({"atMs": null}),
        json!({"atMs": "<absent>"}),
        json!({"review": "approved"}),
        json!({"review": "<absent>"}),
        json!({"note": 7}),
        json!({"note": long("n", 501)}),
        json!({"reviewedBy": 1}),
        json!({"reviewedBy": ""}),
        json!({"reviewedBy": long("p", 129)}),
        json!({"reviewedAtMs": -2}),
        // A null is not a value for a field that has to be one, and `commands` and `n` are whole numbers.
        json!({"n": null}),
        json!({"tool": null}),
        json!({"actor": null}),
        json!({"commands": 1.5}),
    ]
}
pub fn live() -> App {
    let mut a = App::new("user:me", "c0", 1, Vec::new(), 0).unwrap();
    a.dispatch(Action::Connected { now_ms: 0 });
    a.take_events();
    a
}
