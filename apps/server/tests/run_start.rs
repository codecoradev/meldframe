//! The server sees a run start (ADR-0023, slice 4): an agent's write after a gap takes a `runStart` row just before its
//! first op, with no editor open, over REST, the submit socket and MCP; and nothing else does.

mod world;
use meldframe_checkpoints::Kind;
use serde_json::Value;
use world::*;

/// The `(about, seq, by, atMs)` of the rows of a file, newest first.
fn runs(w: &World, ws: &str, file: &str) -> Vec<(String, u64, Option<String>, u64)> {
    let rows = w.rows(ws, file);
    assert!(rows.iter().all(|c| c.kind == Kind::RunStart));
    rows.into_iter()
        .map(|c| (c.about.unwrap(), c.seq, c.by, c.at_ms))
        .collect()
}

fn ok(r: (u16, Value)) {
    assert_eq!(r.0, 200, "{}", r.1);
}

#[tokio::test]
async fn an_agents_write_after_a_gap_takes_a_row_just_before_its_first_op_over_rest() {
    let w = start().await;
    // The person's op is seq 1; the agent's first is seq 2, so the file before the run is seq 1.
    ok(rest(&w, PERSON, "f", vec![add("k0", "p0")]).await);
    assert!(w.rows("ws_1", "f").is_empty());
    ok(rest(&w, AGENT, "f", vec![add("k1", "a1"), add("k2", "a2")]).await);
    assert_eq!(runs(&w, "ws_1", "f"), [("agent:alpha".into(), 1, None, T0)]);
    // A call inside the gap takes none; the gap is counted from the last write, not from the start.
    w.at(T0 + 200_000);
    ok(rest(&w, AGENT, "f", vec![add("k3", "a3")]).await);
    w.at(T0 + 200_000 + GAP);
    ok(rest(&w, AGENT, "f", vec![add("k4", "a4")]).await);
    assert_eq!(
        runs(&w, "ws_1", "f").len(),
        1,
        "exactly five minutes is not a gap"
    );
    // k1, k2, k3, k4 are seq 2 to 5: the next run starts at k5 (seq 6), so the file before it is seq 5.
    w.at(T0 + 200_000 + 2 * GAP + 1);
    ok(rest(&w, AGENT, "f", vec![add("k5", "a5")]).await);
    let rows = runs(&w, "ws_1", "f");
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        ("agent:alpha".into(), 5, None, T0 + 200_000 + 2 * GAP + 1)
    );
}

#[tokio::test]
async fn the_same_over_the_submit_socket() {
    let w = start().await;
    let answer = socket(&w, AGENT, "f", vec![add("k1", "a1")]).await;
    assert_eq!(answer["status"], 200, "{answer}");
    assert_eq!(runs(&w, "ws_1", "f"), [("agent:alpha".into(), 0, None, T0)]);
    w.at(T0 + 10);
    socket(&w, AGENT, "f", vec![add("k2", "a2")]).await;
    assert_eq!(runs(&w, "ws_1", "f").len(), 1);
    w.at(T0 + 10 + GAP + 1);
    socket(&w, AGENT, "f", vec![add("k3", "a3")]).await;
    assert_eq!(runs(&w, "ws_1", "f")[0].1, 2);
}

#[tokio::test]
async fn the_same_over_mcp() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    assert!(
        !call(&c, "apply_commands", apply("f", "k1", "a1"))
            .await
            .is_error
    );
    assert_eq!(runs(&w, "ws_1", "f"), [("agent:alpha".into(), 0, None, T0)]);
    w.at(T0 + 5);
    assert!(
        !call(&c, "apply_commands", apply("f", "k2", "a2"))
            .await
            .is_error
    );
    assert_eq!(runs(&w, "ws_1", "f").len(), 1);
    w.at(T0 + 5 + GAP + 1);
    assert!(
        !call(&c, "apply_commands", apply("f", "k3", "a3"))
            .await
            .is_error
    );
    assert_eq!(runs(&w, "ws_1", "f")[0].1, 2);
}

#[tokio::test]
async fn a_person_never_takes_a_row() {
    let w = start().await;
    for i in 0..3 {
        w.at(T0 + i * 10 * GAP);
        ok(rest(
            &w,
            PERSON,
            "f",
            vec![add(&format!("k{i}"), &format!("p{i}"))],
        )
        .await);
    }
    assert!(w.rows("ws_1", "f").is_empty());
}

#[tokio::test]
async fn a_run_belongs_to_one_agent_in_one_file_of_one_workspace() {
    let w = start().await;
    ok(rest(&w, AGENT, "f", vec![add("k1", "a1")]).await);
    ok(rest(&w, DELTA, "f", vec![add("k1", "d1")]).await);
    ok(rest(&w, AGENT, "g", vec![add("k1", "a1")]).await);
    ok(rest(&w, OTHER, "f", vec![add("k1", "o1")]).await);
    let f: Vec<String> = runs(&w, "ws_1", "f").into_iter().map(|r| r.0).collect();
    assert_eq!(f, ["agent:delta", "agent:alpha"]);
    assert_eq!(runs(&w, "ws_1", "g").len(), 1);
    assert_eq!(runs(&w, "ws_2", "f"), [("agent:gamma".into(), 0, None, T0)]);
}

#[tokio::test]
async fn a_refused_call_claims_no_run() {
    let w = start().await;
    let (status, _) = rest(&w, AGENT, "f", vec![bad("k1")]).await;
    assert_ne!(status, 200);
    assert!(w.rows("ws_1", "f").is_empty());
    // The next good call is still the start of the run.
    ok(rest(&w, AGENT, "f", vec![add("k2", "a2")]).await);
    assert_eq!(runs(&w, "ws_1", "f").len(), 1);
}

#[tokio::test]
async fn a_replay_is_not_a_write_and_starts_nothing() {
    let w = start().await;
    ok(rest(&w, AGENT, "f", vec![add("k1", "a1")]).await);
    w.at(T0 + 2 * GAP);
    // The same key again: the answer says nothing was written, and the run does not start again.
    ok(rest(&w, AGENT, "f", vec![add("k1", "a1")]).await);
    assert_eq!(runs(&w, "ws_1", "f").len(), 1);
    ok(rest(&w, AGENT, "f", vec![add("k2", "a2")]).await);
    let rows = runs(&w, "ws_1", "f");
    assert_eq!((rows.len(), rows[0].1), (2, 1));
}

#[tokio::test]
async fn a_call_that_wrote_two_ops_and_failed_on_the_third_still_takes_the_row() {
    let w = start().await;
    ok(rest(&w, PERSON, "f", vec![add("p", "p0")]).await);
    w.cut.cut.store(2, std::sync::atomic::Ordering::SeqCst);
    let (status, _) = rest(
        &w,
        AGENT,
        "f",
        vec![add("k1", "a1"), add("k2", "a2"), add("k3", "a3")],
    )
    .await;
    assert_ne!(status, 200, "the call failed");
    w.cut.cut.store(0, std::sync::atomic::Ordering::SeqCst);
    // The two ops that were written are seq 2 and 3: the file before the run is seq 1.
    assert_eq!(runs(&w, "ws_1", "f"), [("agent:alpha".into(), 1, None, T0)]);
    // The run goes on: a call inside the gap takes none.
    ok(rest(&w, AGENT, "f", vec![add("k4", "a4")]).await);
    assert_eq!(runs(&w, "ws_1", "f").len(), 1);
}

#[tokio::test]
async fn a_failed_call_that_wrote_nothing_claims_no_run() {
    use std::sync::atomic::Ordering;
    let w = start().await;
    // The submitter writes the call's first op, which the room refuses, and fails: the sink looks for what was written
    // by key, finds nothing, and leaves the detector alone.
    w.cut.cut.store(1, Ordering::SeqCst);
    let (status, _) = rest(&w, AGENT, "f", vec![bad("k1")]).await;
    assert_ne!(status, 200);
    w.cut.cut.store(0, Ordering::SeqCst);
    assert!(w.rows("ws_1", "f").is_empty());
    // So the first call that does write is still the start of the run.
    ok(rest(&w, AGENT, "f", vec![add("k2", "a2")]).await);
    assert_eq!(runs(&w, "ws_1", "f").len(), 1);
}

#[tokio::test]
async fn calls_of_one_agent_at_once_make_exactly_one_row() {
    let w = std::sync::Arc::new(start().await);
    let tasks: Vec<_> = (0..4)
        .map(|i| {
            let w = w.clone();
            tokio::spawn(async move {
                let op = add(&format!("k{i}"), &format!("a{i}"));
                rest(&w, AGENT, "f", vec![op]).await.0
            })
        })
        .collect();
    for t in tasks {
        assert_eq!(t.await.unwrap(), 200);
    }
    assert_eq!(runs(&w, "ws_1", "f").len(), 1);
}

#[tokio::test]
async fn a_restarted_detector_counts_the_next_write_as_a_start() {
    let w = start().await;
    ok(rest(&w, AGENT, "f", vec![add("k1", "a1")]).await);
    assert_eq!(runs(&w, "ws_1", "f").len(), 1);
    // The process restarts inside the gap: the files and the rows are kept and the detector is empty, so the next write
    // is one extra row, as decision 39 took (question 7, option a).
    let again = w.restarted().await;
    ok(rest(&again, AGENT, "f", vec![add("k2", "a2")]).await);
    let rows = runs(&again, "ws_1", "f");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].1, 1, "the file as it was before that write");
    ok(rest(&again, AGENT, "f", vec![add("k3", "a3")]).await);
    assert_eq!(
        runs(&again, "ws_1", "f").len(),
        2,
        "and then it is a run again"
    );
}

#[tokio::test]
async fn a_slow_store_does_not_hold_the_reply_past_the_time_limit_and_the_run_is_not_lost() {
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    let w = start().await;
    // What the same call answers with a store that is not slow.
    let (_, plain) = rest(&w, AGENT, "plain", vec![add("k1", "a1")]).await;
    // The store takes 700 ms; the call's time limit is 400 ms.
    w.store.slow.store(true, Ordering::SeqCst);
    let started = Instant::now();
    let (status, body) = rest(&w, AGENT, "f", vec![add("k1", "a1")]).await;
    let took = started.elapsed();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, plain, "the reply is the one a fast store gives");
    assert!(
        took < Duration::from_millis(650),
        "the reply waited for the store: {took:?}"
    );
    // The job outlasted the limit: logged and counted, and the claim was given back.
    assert_eq!(w.runs.failures(), 1);
    w.store.slow.store(false, Ordering::SeqCst);
    for _ in 0..500 {
        if w.rows("ws_1", "f").len() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        runs(&w, "ws_1", "f").len(),
        1,
        "the slow write finished and made its row"
    );
    // The next call inside the run takes the run start (the claim was given back): one extra row, never a lost run.
    w.at(T0 + 10);
    ok(rest(&w, AGENT, "f", vec![add("k2", "a2")]).await);
    assert_eq!(
        runs(&w, "ws_1", "f")
            .iter()
            .map(|r| r.1)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    // And a call after the gap takes one as always.
    w.at(T0 + 10 + GAP + 1);
    ok(rest(&w, AGENT, "f", vec![add("k3", "a3")]).await);
    assert_eq!(
        runs(&w, "ws_1", "f")
            .iter()
            .map(|r| r.1)
            .collect::<Vec<_>>(),
        [2, 1, 0]
    );
    assert_eq!(w.runs.failures(), 1, "nothing else failed");
}

#[tokio::test]
async fn a_store_that_fails_changes_no_reply_is_counted_and_gives_the_claim_back() {
    use std::sync::atomic::Ordering;
    let w = start().await;
    w.store.fail.store(true, Ordering::SeqCst);
    let (status, body) = rest(&w, AGENT, "f", vec![add("k1", "a1")]).await;
    assert_eq!(
        (status, body["accepted"][0]["seq"].clone()),
        (200, 1.into()),
        "{body}"
    );
    assert!(w.rows("ws_1", "f").is_empty());
    assert_eq!(w.runs.failures(), 1);
    // The file was written all the same, and the next call inside the run takes the run start the failure lost.
    w.store.fail.store(false, Ordering::SeqCst);
    ok(rest(&w, AGENT, "f", vec![add("k2", "a2")]).await);
    assert_eq!(runs(&w, "ws_1", "f"), [("agent:alpha".into(), 1, None, T0)]);
    ok(rest(&w, AGENT, "f", vec![add("k3", "a3")]).await);
    assert_eq!(runs(&w, "ws_1", "f").len(), 1, "and then the run goes on");
    assert_eq!(w.runs.failures(), 1);
}

#[tokio::test]
async fn the_two_calls_of_a_person_are_not_mcp_tools() {
    let w = start().await;
    let c = connect(&w.h, PERSON).await;
    let tools = c.list_all_tools().await.unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"apply_commands"));
    assert!(
        !names.iter().any(|n| n.starts_with("checkpoint")),
        "{names:?}"
    );
}
