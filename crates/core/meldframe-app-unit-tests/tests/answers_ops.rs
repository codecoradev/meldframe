//! The answer of the server to an op it did not accept, read in the app layer (audit finding T14, issue 324). The shell
//! hands over the status and the text of the body; the app takes the code and the retry flag from it, and when the body is
//! not what the server writes (a proxy's page, a field of the wrong type or none) the op was refused for now: the code is
//! `unavailable` and the flag is "retryable", so the app sends the op again by its own rule and gives up after three tries.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, Connection, Effect, Event, Frame};
use meldframe_app_model::Wire;
use meldframe_app_parts::answers::action_of;
use serde_json::json;

mod common;
use common::{Out, app, get};

fn refused(status: u16, body: &str) -> Wire {
    Wire::OpRefused {
        now_ms: 7,
        key: "k1".into(),
        status,
        body: body.into(),
    }
}

fn rejected(code: &str, retryable: bool) -> Out {
    Out(Some(Action::Frame {
        now_ms: 7,
        frame: Frame::Rejected {
            key: "k1".into(),
            code: code.into(),
            retryable,
        },
    }))
}

fn read(status: u16, body: &str) -> Out {
    Out(action_of(refused(status, body)))
}

#[test]
fn the_code_and_the_flag_of_the_error_are_taken_as_the_server_wrote_them() {
    for (status, code, retryable) in [
        (409, "conflict", false),
        (429, "quota_ops_per_minute", true),
        (403, "approval_required", false),
        (413, "quota_storage", false),
        (400, "E_NODE_NOT_FOUND", false),
        (400, "E_TOO_LONG", false),
        (503, "unavailable", true),
    ] {
        let body = json!({"error": {"code": code, "message": "m", "retryable": retryable}});
        assert_eq!(
            read(status, &body.to_string()),
            rejected(code, retryable),
            "{code}"
        );
    }
}

#[test]
fn an_error_with_no_retry_flag_is_retryable() {
    assert_eq!(
        read(500, r#"{"error":{"code":"internal","message":"m"}}"#),
        rejected("internal", true)
    );
    assert_eq!(
        read(409, r#"{"error":{"code":"E_INVALID_ID"}}"#),
        rejected("E_INVALID_ID", true)
    );
    for flag in [json!("no"), json!(0), json!(null), json!([]), json!({})] {
        let body = json!({"error": {"code": "conflict", "retryable": flag}});
        assert_eq!(
            read(409, &body.to_string()),
            rejected("conflict", true),
            "{flag}"
        );
    }
}

#[test]
fn a_body_that_is_not_what_the_server_writes_is_unavailable_and_retryable() {
    for body in [
        "",
        "<html>bad gateway</html>",
        "null",
        "[]",
        "5",
        r#""x""#,
        "{}",
        r#"{"error":"x"}"#,
        r#"{"error":null}"#,
        r#"{"error":[]}"#,
        r#"{"error":{}}"#,
        r#"{"error":{"code":5,"retryable":"x"}}"#,
        r#"{"error":{"code":null}}"#,
        r#"{"accepted":[]}"#,
    ] {
        assert_eq!(read(502, body), rejected("unavailable", true), "{body}");
    }
    // the flag is read on its own: a body with a flag and no code keeps the flag
    assert_eq!(
        read(502, r#"{"error":{"retryable":false}}"#),
        rejected("unavailable", false)
    );
}

/// The two edges where `serde_json` and the `JSON.parse` the shell used to read the body with disagree: it refuses a lone
/// surrogate escape and a nesting over 128 levels, `JSON.parse` takes both. A server or a proxy sends neither in practice;
/// such a body is not read, so it is `unavailable` and retryable instead of carrying its own code.
#[test]
fn a_body_that_only_javascript_reads_is_unavailable_and_retryable() {
    let lone_surrogate = r#"{"error":{"code":"conflict\ud800","retryable":false}}"#;
    assert_eq!(read(409, lone_surrogate), rejected("unavailable", true));
    let deep = format!(
        r#"{{"error":{{"code":"conflict","retryable":false,"x":{}{}}}}}"#,
        "[".repeat(200),
        "]".repeat(200)
    );
    assert_eq!(read(409, &deep), rejected("unavailable", true));
    // a nesting inside the limit is read
    let shallow = format!(
        r#"{{"error":{{"code":"conflict","retryable":false,"x":{}{}}}}}"#,
        "[".repeat(50),
        "]".repeat(50)
    );
    assert_eq!(read(409, &shallow), rejected("conflict", false));
}

#[test]
fn a_field_the_app_does_not_know_is_ignored() {
    let body = json!({"error": {"code": "conflict", "retryable": false, "message": "m", "details": [1, 2], "retryAfterMs": 5}, "extra": true});
    assert_eq!(read(409, &body.to_string()), rejected("conflict", false));
}

#[test]
fn only_a_200_is_an_op_that_was_accepted_and_every_other_status_is_a_refusal() {
    assert_eq!(read(200, r#"{"accepted":[{"key":"k1"}]}"#), Out(None));
    assert_eq!(read(200, ""), Out(None));
    assert_eq!(
        read(200, r#"{"error":{"code":"conflict","retryable":false}}"#),
        Out(None)
    );
    for status in [201, 202, 204, 301, 400, 404, 500, 0] {
        assert_eq!(read(status, ""), rejected("unavailable", true), "{status}");
    }
}

/// An app that is live with one op of its own on the way.
fn with_op() -> (meldframe_app::App, String) {
    let mut app = app();
    app.dispatch(Action::Start { now_ms: 0 });
    app.dispatch(Action::Connected { now_ms: 0 });
    let fx = app.dispatch(Action::Move {
        id: "r1".into(),
        x: 3.0,
        y: 0.0,
    });
    let key = fx
        .iter()
        .find_map(|e| match e {
            Effect::SendOps { ops } => Some(ops[0].key.clone()),
            _ => None,
        })
        .expect("the move is sent");
    (app, key)
}

fn answer(key: &str, now_ms: u64, status: u16, body: &str) -> Action {
    Action::Wire {
        wire: Wire::OpRefused {
            now_ms,
            key: key.into(),
            status,
            body: body.into(),
        },
    }
}

fn sent_keys(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.key.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

#[test]
fn an_answer_that_is_not_the_servers_is_retried_by_the_app_three_times_with_the_same_key_then_rolled_back()
 {
    let (mut app, key) = with_op();
    let mut now = 100;
    for attempt in 1..=3u32 {
        let fx = app.dispatch(answer(&key, now, 502, "<html>bad gateway</html>"));
        assert!(
            fx.iter()
                .any(|e| matches!(e, Effect::WakeAfter { after_ms } if *after_ms > 0))
        );
        assert!(app.take_events().contains(&Event::OpRetry {
            key: key.clone(),
            attempt
        }));
        assert_eq!(
            get(&app, "r1").x,
            3.0,
            "the view keeps the edit while it waits"
        );
        now += 10_000;
        assert_eq!(
            sent_keys(&app.dispatch(Action::Tick { now_ms: now })),
            std::slice::from_ref(&key)
        );
    }
    app.dispatch(answer(&key, now, 502, "<html>bad gateway</html>"));
    assert!(app.take_events().contains(&Event::OpRejected {
        key: key.clone(),
        code: "unavailable".into(),
        node: None,
        retryable: true
    }));
    assert_eq!(get(&app, "r1").x, 0.0);
    assert!(app.state().pending.is_empty());
}

#[test]
fn a_refusal_for_good_rolls_back_at_once_under_the_code_the_server_wrote() {
    for code in ["conflict", "E_TOO_LONG"] {
        let (mut app, key) = with_op();
        let body = json!({"error": {"code": code, "message": "m", "retryable": false}});
        let fx = app.dispatch(answer(&key, 5, 409, &body.to_string()));
        assert!(!fx.iter().any(|e| matches!(e, Effect::WakeAfter { .. })));
        assert!(app.take_events().contains(&Event::OpRejected {
            key,
            code: code.into(),
            node: None,
            retryable: false
        }));
        assert_eq!(get(&app, "r1").x, 0.0);
    }
}

#[test]
fn the_code_is_made_safe_where_it_enters_the_app() {
    let (mut app, key) = with_op();
    let body = json!({"error": {"code": "con\u{202e}flict\u{200b}", "retryable": false}});
    app.dispatch(answer(&key, 5, 409, &body.to_string()));
    assert!(app.take_events().contains(&Event::OpRejected {
        key,
        code: "conflict".into(),
        node: None,
        retryable: false
    }));
}

#[test]
fn an_answer_for_an_op_that_is_not_waiting_changes_nothing() {
    let (mut app, key) = with_op();
    let before = serde_json::to_value(app.state()).unwrap();
    assert!(
        app.dispatch(answer(
            "no-such-key",
            5,
            409,
            r#"{"error":{"code":"conflict","retryable":false}}"#
        ))
        .is_empty()
    );
    // an accepted answer (200) is nothing too: the live event confirms the op
    assert!(
        app.dispatch(answer(&key, 5, 200, r#"{"accepted":[]}"#))
            .is_empty()
    );
    assert_eq!(serde_json::to_value(app.state()).unwrap(), before);
    assert_eq!(app.state().connection, Connection::Live);
    assert_eq!(app.state().pending, [key]);
}
