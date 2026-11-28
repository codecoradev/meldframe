//! The `note` of `apply_commands` and the header it makes, through a real MCP client against the real server
//! (ADR-0021, slice 4): the check before the write, the three cases (a call that wrote, a call that failed after a
//! prefix, a replay), the answer and what is kept.

mod activity_world;
use activity_world::*;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn a_call_with_a_note_makes_a_numbered_batch_and_answers_it() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        apply(
            "f",
            "k1",
            vec![frame("a", "A")],
            json!({"note": "  Hero: new title\r\nwhy: brief  "}),
        ),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["batch"], json!({"n": 1, "note": "kept"}));
    let r = call(
        &c,
        "apply_commands",
        apply("f", "k2", vec![frame("b", "B")], json!({})),
    )
    .await;
    assert_eq!(r.body["batch"], json!({"n": 2, "note": "none"}));
    let h = headers(&w, "ws_1", "f");
    assert_eq!(h.len(), 2);
    let first = h.iter().find(|h| h.n == 1).unwrap();
    assert_eq!(
        first.note.as_ref().unwrap().as_str(),
        "Hero: new title\nwhy: brief"
    );
    assert_eq!(
        (first.actor.as_str(), first.key_prefix.as_str()),
        ("agent:alpha", "k1")
    );
    assert_eq!((first.commands, first.written, first.atomic), (1, 1, true));
    assert_eq!(first.tool, "apply_commands");
}

#[tokio::test]
async fn a_bad_note_refuses_the_call_and_writes_nothing() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    let bad = [
        "a".repeat(501),
        "   \n ".to_owned(),
        "one\ntwo\nthree\nfour\nfive".to_owned(),
        "hid\u{200B}den".to_owned(),
        "tag\u{E0041}".to_owned(),
        "emoji \u{1F468}\u{200D}\u{1F469}".to_owned(),
        "ctl\u{7}".to_owned(),
        "e\u{301}\u{301}\u{301}\u{301}\u{301}".to_owned(),
    ];
    for note in bad {
        let r = call(
            &c,
            "apply_commands",
            apply("f", "k", vec![frame("a", "A")], json!({"note": note})),
        )
        .await;
        assert!(r.is_error, "{note:?}");
        assert_eq!(r.body["error"]["code"], "invalid", "{note:?}");
    }
    let tree = call(&c, "get_tree", json!({"file": "f"})).await;
    assert_eq!(tree.body["seq"], 0);
    assert!(headers(&w, "ws_1", "f").is_empty());
    // The key was never used: the same call without a note writes.
    let ok = call(
        &c,
        "apply_commands",
        apply("f", "k", vec![frame("a", "A")], json!({})),
    )
    .await;
    assert!(!ok.is_error, "{}", ok.body);
}

#[tokio::test]
async fn a_non_atomic_call_stopped_at_the_third_command_records_two_of_five() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    let cmds = vec![
        frame("a", "A"),
        frame("b", "B"),
        json!({"type": "updateNode", "id": "ghost", "patch": {"x": 1}}),
        frame("d", "D"),
        frame("e", "E"),
    ];
    let r = call(
        &c,
        "apply_commands",
        apply(
            "f",
            "part",
            cmds,
            json!({"atomic": false, "note": "five frames"}),
        ),
    )
    .await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["batch"], json!({"n": 1, "written": 2}));
    let h = headers(&w, "ws_1", "f");
    assert_eq!((h[0].commands, h[0].written, h[0].atomic), (5, 2, false));
    assert_eq!((h[0].first_seq, h[0].last_seq), (1, 2));
    assert_eq!(h[0].note.as_ref().unwrap().as_str(), "five frames");
}

#[tokio::test]
async fn a_call_that_wrote_nothing_has_no_number() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        apply(
            "f",
            "nope",
            vec![json!({"type": "updateNode", "id": "ghost", "patch": {"x": 1}})],
            json!({"atomic": false}),
        ),
    )
    .await;
    assert!(r.is_error);
    assert!(r.body["error"].get("batch").is_none(), "{}", r.body);
    assert!(headers(&w, "ws_1", "f").is_empty());
}

#[tokio::test]
async fn a_timed_out_call_is_healed_by_its_retry_with_the_time_of_the_replay() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    w.slow.slow.store(true, Ordering::SeqCst);
    let args = apply(
        "f",
        "slow",
        vec![frame("a", "A"), frame("b", "B")],
        json!({"note": "slow one"}),
    );
    let r = call(&c, "apply_commands", args.clone()).await;
    assert_eq!(r.body["error"]["code"], "timeout", "{}", r.body);
    w.slow.ended_after(0).await;
    assert!(
        headers(&w, "ws_1", "f").is_empty(),
        "a timed out call keeps no header"
    );
    w.slow.slow.store(false, Ordering::SeqCst);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let r = call(&c, "apply_commands", args).await;
    assert_eq!(r.body["replayed"], true, "{}", r.body);
    assert_eq!(r.body["batch"], json!({"n": 1, "note": "kept"}));
    let h = headers(&w, "ws_1", "f");
    assert_eq!((h[0].commands, h[0].written), (2, 2));
    assert!(
        h[0].at_ms >= now,
        "the header carries the time of the replay"
    );
    // A second replay changes nothing and keeps the first note.
    let again = apply(
        "f",
        "slow",
        vec![frame("a", "A"), frame("b", "B")],
        json!({"note": "other"}),
    );
    let r = call(&c, "apply_commands", again).await;
    assert_eq!(r.body["batch"], json!({"n": 1, "note": "kept_earlier"}));
    assert_eq!(headers(&w, "ws_1", "f")[0].written, 2);
}

#[tokio::test]
async fn a_later_note_is_ignored_and_the_answer_says_so() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    let first = apply("f", "same", vec![frame("a", "A")], json!({"note": "first"}));
    call(&c, "apply_commands", first).await;
    let again = apply(
        "f",
        "same",
        vec![frame("a", "A")],
        json!({"note": "second"}),
    );
    let r = call(&c, "apply_commands", again).await;
    assert_eq!(r.body["batch"], json!({"n": 1, "note": "kept_earlier"}));
    assert_eq!(
        headers(&w, "ws_1", "f")[0].note.as_ref().unwrap().as_str(),
        "first"
    );
}

#[tokio::test]
async fn a_denied_batch_status_still_records_headers_and_the_call_log_shows_no_extra_reads() {
    let w = start(true, true).await;
    let c = connect(&w.h, AGENT).await;
    let cmds = vec![
        frame("a", "A"),
        json!({"type": "updateNode", "id": "ghost", "patch": {"x": 1}}),
    ];
    let r = call(
        &c,
        "apply_commands",
        apply("f", "d", cmds, json!({"atomic": false})),
    )
    .await;
    assert_eq!(
        r.body["error"]["batch"],
        json!({"n": 1, "written": 1}),
        "{}",
        r.body
    );
    let tools: Vec<String> = w.server.calls(100).into_iter().map(|e| e.tool).collect();
    assert!(tools.iter().all(|t| t == "apply_commands"), "{tools:?}");
}

#[tokio::test]
async fn another_workspace_never_sees_the_headers() {
    let w = start(true, false).await;
    let a = connect(&w.h, AGENT).await;
    let g = connect(&w.h, OTHER).await;
    call(
        &a,
        "apply_commands",
        apply("f", "k", vec![frame("a", "A")], json!({"note": "mine"})),
    )
    .await;
    let r = call(
        &g,
        "apply_commands",
        apply("f", "k", vec![frame("a", "A")], json!({})),
    )
    .await;
    assert_eq!(r.body["batch"], json!({"n": 1, "note": "none"}));
    assert_eq!(headers(&w, "ws_1", "f").len(), 1);
    assert_eq!(headers(&w, "ws_2", "f").len(), 1);
    assert!(headers(&w, "ws_2", "f")[0].note.is_none());
}

#[tokio::test]
async fn the_call_log_holds_no_note() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    call(
        &c,
        "apply_commands",
        apply(
            "f",
            "k",
            vec![frame("a", "A")],
            json!({"note": "secret-note-text"}),
        ),
    )
    .await;
    let log = format!("{:?}", w.server.calls(100));
    assert!(log.contains("apply_commands"));
    assert!(!log.contains("secret-note-text"), "{log}");
}

#[tokio::test]
async fn without_the_feature_a_note_is_refused_and_a_call_without_one_still_works() {
    let w = start(false, false).await;
    let c = connect(&w.h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        apply("f", "k", vec![frame("a", "A")], json!({"note": "hello"})),
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "invalid");
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("keeps no notes")
    );
    let r = call(
        &c,
        "apply_commands",
        apply("f", "k", vec![frame("a", "A")], json!({"note": ""})),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["batch"], Value::Null);
}

#[tokio::test]
async fn the_note_is_in_the_schema_and_the_text_even_when_the_server_keeps_none() {
    let w = start(false, false).await;
    let c = connect(&w.h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    let t = tools.iter().find(|t| t.name == "apply_commands").unwrap();
    assert!(
        t.input_schema
            .get("properties")
            .unwrap()
            .get("note")
            .is_some()
    );
    assert!(t.description.as_deref().unwrap().contains("note"));
}
