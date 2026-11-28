//! The `checkpoint` frame on the live streams of the real server (ADR-0023, slice 5): where it comes in the queue, what
//! it holds, who gets it, and that neither a flood by agents nor a burst of creates takes a person's row or delays an op.

mod checkpoint_world;
use checkpoint_world::*;
use meldframe_checkpoints::Kind;
use meldframe_host::Principal;
use meldframe_http::live::LiveSource;
use meldframe_protocol::ServerMsg;
use meldframe_server::Frame;
use serde_json::json;

type Out = Frame<meldframe_host::StoredOp, ServerMsg>;

fn who(ws: &str, actor: &str) -> Principal {
    Principal::new(ws, actor)
}

fn drain(w: &World, p: &Principal, file: &str, id: meldframe_server::StreamId) -> Vec<Out> {
    w.server.poll(p, file, id, 0, 100_000)
}

fn checkpoints(frames: &[Out]) -> Vec<&ServerMsg> {
    fn hint(f: &Out) -> Option<&ServerMsg> {
        match f {
            Frame::Ephemeral(m @ ServerMsg::Checkpoint { .. }) => Some(m),
            _ => None,
        }
    }
    frames.iter().filter_map(hint).collect()
}

fn resyncs(frames: &[Out]) -> usize {
    frames
        .iter()
        .filter(|f| matches!(f, Frame::Resync { .. }))
        .count()
}

fn durable(frames: &[Out]) -> usize {
    frames
        .iter()
        .filter(|f| matches!(f, Frame::Durable { .. }))
        .count()
}

async fn create(w: &World, key: &str, extra: serde_json::Value) -> u16 {
    let mut b = json!({"key": key, "kind": "manual"});
    for (k, v) in extra.as_object().unwrap() {
        b[k] = v.clone();
    }
    post(w, Some(PERSON), "/v1/files/f/checkpoints", &b.to_string())
        .await
        .0
}

#[tokio::test]
async fn the_frame_follows_the_creating_call_and_holds_the_row() {
    let w = start().await;
    assert_eq!(rest(&w, PERSON, "f", vec![add("k0", "p0")]).await.0, 200);
    let reader = who("ws_1", "user:bea");
    let id = w.server.subscribe(&reader, "f", 0, 0).unwrap().id;
    let stranger = w
        .server
        .subscribe(&who("ws_2", "agent:gamma"), "f", 0, 0)
        .unwrap()
        .id;
    assert_eq!(create(&w, "k", json!({"label": "v1"})).await, 200);
    let got = drain(&w, &reader, "f", id);
    let ServerMsg::Checkpoint {
        room,
        n,
        seq,
        kind,
        label,
        about,
        by,
        at_ms,
    } = checkpoints(&got)[0].clone()
    else {
        unreachable!()
    };
    assert_eq!(
        (room.as_str(), n, seq, kind.as_str()),
        ("f", 1, 1, "manual")
    );
    assert_eq!(
        (label.as_deref(), about, by.as_deref(), at_ms),
        (Some("v1"), None, Some("user:bea"), T0)
    );
    // Another workspace's stream gets nothing of it.
    assert!(checkpoints(&drain(&w, &who("ws_2", "agent:gamma"), "f", stranger)).is_empty());
}

#[tokio::test]
async fn a_run_start_is_announced_after_the_ops_of_its_call() {
    let w = start().await;
    let reader = who("ws_1", "user:bea");
    let id = w.server.subscribe(&reader, "f", 0, 0).unwrap().id;
    assert_eq!(
        rest(&w, AGENT, "f", vec![add("k1", "a1"), add("k2", "a2")])
            .await
            .0,
        200
    );
    let got = drain(&w, &reader, "f", id);
    let last = got.last().unwrap();
    let Frame::Ephemeral(ServerMsg::Checkpoint {
        n,
        seq,
        kind,
        about,
        by,
        label,
        ..
    }) = last
    else {
        panic!("the frame comes last: {} frames", got.len());
    };
    assert_eq!((*n, *seq, kind.as_str()), (1, 0, "runStart"));
    assert_eq!(
        (about.as_deref(), by.clone(), label.clone()),
        (Some("agent:alpha"), None, None)
    );
    assert_eq!(durable(&got), 2);
}

#[tokio::test]
async fn an_agent_flood_and_refused_creates_evict_no_persons_row_and_block_no_create() {
    let w = start().await;
    assert_eq!(rest(&w, PERSON, "f", vec![add("p", "p0")]).await.0, 200);
    assert_eq!(create(&w, "keep", json!({"label": "keep me"})).await, 200);
    // Thirty runs of the agent, each a gap apart, and forty creates by the agent, which are refused.
    for i in 0..30u64 {
        w.at(T0 + (i + 1) * (GAP + 1));
        let op = add(&format!("a{i}"), &format!("n{i}"));
        assert_eq!(rest(&w, AGENT, "f", vec![op]).await.0, 200);
        let r = post(
            &w,
            Some(AGENT),
            "/v1/files/f/checkpoints",
            &json!({"key": format!("x{i}"), "kind": "manual"}).to_string(),
        )
        .await;
        assert_eq!(r.0, 403);
    }
    let rows = w.rows("ws_1", "f");
    let runs = rows.iter().filter(|c| c.kind == Kind::RunStart).count();
    assert_eq!(runs, 10, "one agent holds at most ten run starts");
    assert!(
        rows.iter()
            .any(|c| c.label.as_ref().is_some_and(|l| l.as_str() == "keep me"))
    );
    assert_eq!(
        create(&w, "after", json!({})).await,
        200,
        "the person's own create still works"
    );
}

#[tokio::test]
async fn a_burst_of_creates_never_pushes_a_durable_op_into_a_resync() {
    let w = start().await;
    let reader = who("ws_1", "user:bea");
    let id = w.server.subscribe(&reader, "f", 0, 0).unwrap().id;
    // 60 ops, 20 creates (the cap of a person), 60 more ops and 10 runs of the agent, into a queue that nobody reads.
    for i in 0..60 {
        assert_eq!(
            rest(
                &w,
                PERSON,
                "f",
                vec![add(&format!("a{i}"), &format!("a{i}"))]
            )
            .await
            .0,
            200
        );
    }
    for i in 0..20 {
        assert_eq!(create(&w, &format!("c{i}"), json!({})).await, 200);
    }
    for i in 0..60 {
        assert_eq!(
            rest(
                &w,
                PERSON,
                "f",
                vec![add(&format!("b{i}"), &format!("b{i}"))]
            )
            .await
            .0,
            200
        );
    }
    for i in 0..10u64 {
        w.at(T0 + (i + 1) * (GAP + 1));
        assert_eq!(
            rest(
                &w,
                AGENT,
                "f",
                vec![add(&format!("r{i}"), &format!("r{i}"))]
            )
            .await
            .0,
            200
        );
    }
    let got = drain(&w, &reader, "f", id);
    assert_eq!(resyncs(&got), 0, "a durable op was delayed into a resync");
    assert_eq!(durable(&got), 130);
    assert_eq!(checkpoints(&got).len(), 30);
}
