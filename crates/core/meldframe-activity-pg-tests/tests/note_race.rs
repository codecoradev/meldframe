//! Calls of one batch that overlap or repeat (ADR-0021, slice 4): a replay racing the original it replays, a retry that
//! fails again, and a non-atomic call that times out. The sink is driven in the order the test chooses.

mod world;
use meldframe_http_activity::Activity;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use world::*;

/// The sink as `apply_commands` calls it, without the transport: the order of two calls of one batch is the test's.
fn sink_of(w: &World) -> Arc<dyn meldframe_activity::BatchSink> {
    Activity::new(w.store.clone(), w.server.clone()).sink()
}

fn call_of(file: &str, key: &str, commands: usize) -> meldframe_activity::BatchCall {
    meldframe_activity::BatchCall {
        file: file.into(),
        key: key.into(),
        tool: "apply_commands",
        commands,
        atomic: true,
        note: None,
    }
}

#[tokio::test]
async fn a_replay_and_a_delayed_original_count_each_op_once_in_either_order() {
    let w = start(true, false).await;
    let sink = sink_of(&w);
    // Five commands: an earlier try wrote two, the original wrote the other three (flags true) and is slow to reach the
    // sink, and the replay found all five (flags false). Each has a different count of ops it wrote itself, so adding
    // the counts gives 3 or 0 and only the keys the log holds give 5.
    let replay = [(1, false), (2, false), (3, false), (4, false), (5, false)];
    let original = [(1, false), (2, false), (3, true), (4, true), (5, true)];
    for (key, order) in [
        ("race", [&replay, &original]),
        ("race2", [&original, &replay]),
    ] {
        let call = call_of("f", key, 5);
        for done in order {
            sink.record("ws_1", "agent:alpha", &call, Some(done))
                .unwrap();
        }
        let h = headers(&w, "ws_1", "f");
        let h = h.iter().find(|h| h.key_prefix == key).unwrap();
        assert_eq!((h.commands, h.written), (5, 5), "{key}: {h:?}");
    }
}

#[tokio::test]
async fn a_retry_that_fails_again_raises_written_to_what_the_log_holds() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    let bad = json!({"type": "updateNode", "id": "ghost", "patch": {"x": 1}});
    let first = vec![
        frame("a", "A"),
        frame("b", "B"),
        bad.clone(),
        frame("d", "D"),
    ];
    let r = call(
        &c,
        "apply_commands",
        apply("f", "again", first, json!({"atomic": false})),
    )
    .await;
    assert_eq!(
        r.body["error"]["batch"],
        json!({"n": 1, "written": 2}),
        "{}",
        r.body
    );
    // The same key: two ops replay, one more is written, and the call stops again.
    let second = vec![
        frame("a", "A"),
        frame("b", "B"),
        frame("c", "C"),
        bad,
        frame("d", "D"),
    ];
    let r = call(
        &c,
        "apply_commands",
        apply("f", "again", second, json!({"atomic": false})),
    )
    .await;
    assert_eq!(
        r.body["error"]["batch"],
        json!({"n": 1, "written": 3}),
        "{}",
        r.body
    );
    let h = headers(&w, "ws_1", "f");
    assert_eq!((h[0].commands, h[0].written), (5, 3), "{h:?}");
}

#[tokio::test]
async fn a_non_atomic_call_that_times_out_keeps_no_partial_header_and_its_retry_makes_the_whole_one()
 {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    w.slow.slow.store(true, Ordering::SeqCst);
    let args = apply(
        "f",
        "slow2",
        vec![frame("a", "A"), frame("b", "B")],
        json!({"atomic": false}),
    );
    let r = call(&c, "apply_commands", args.clone()).await;
    assert_eq!(r.body["error"]["code"], "timeout", "{}", r.body);
    w.slow.ended_after(0).await;
    assert!(headers(&w, "ws_1", "f").is_empty());
    w.slow.slow.store(false, Ordering::SeqCst);
    let r = call(&c, "apply_commands", args).await;
    assert_eq!(
        r.body["batch"],
        json!({"n": 1, "note": "none"}),
        "{}",
        r.body
    );
    let h = headers(&w, "ws_1", "f");
    assert_eq!((h[0].commands, h[0].written, h[0].atomic), (2, 2, false));
}

#[tokio::test]
async fn an_atomic_call_whose_store_failed_part_way_answers_error_batch() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    w.slow.cut.store(2, Ordering::SeqCst);
    let cmds = vec![
        frame("a", "A"),
        frame("b", "B"),
        frame("c", "C"),
        frame("d", "D"),
    ];
    let r = call(
        &c,
        "apply_commands",
        apply("f", "cut", cmds, json!({"note": "four"})),
    )
    .await;
    assert_eq!(r.body["error"]["code"], "unavailable", "{}", r.body);
    assert_eq!(
        r.body["error"]["batch"],
        json!({"n": 1, "written": 2}),
        "{}",
        r.body
    );
    let h = headers(&w, "ws_1", "f");
    assert_eq!((h[0].commands, h[0].written, h[0].atomic), (4, 2, true));
}

#[tokio::test]
async fn an_atomic_call_that_fails_for_good_or_is_refused_keeps_no_header() {
    let w = start(true, false).await;
    let c = connect(&w.h, AGENT).await;
    // A permanent store failure is `invalid`, not a 503: it may have written a prefix, and the retry heals it.
    w.slow.cut.store(2, Ordering::SeqCst);
    w.slow.permanent.store(true, Ordering::SeqCst);
    let cmds = vec![frame("a", "A"), frame("b", "B"), frame("c", "C")];
    let r = call(&c, "apply_commands", apply("f", "perm", cmds, json!({}))).await;
    assert_eq!(r.body["error"]["code"], "invalid", "{}", r.body);
    assert!(r.body["error"].get("batch").is_none(), "{}", r.body);
    w.slow.cut.store(0, Ordering::SeqCst);
    // A call the gate refuses writes nothing and so has no header.
    let stale = json!({"ifSeq": 0});
    let bad = json!({"type": "updateNode", "id": "a", "patch": {"x": 1}});
    let r = call(&c, "apply_commands", apply("f", "stale", vec![bad], stale)).await;
    assert!(r.is_error, "{}", r.body);
    assert!(r.body["error"].get("batch").is_none(), "{}", r.body);
    assert!(headers(&w, "ws_1", "f").is_empty());
}
