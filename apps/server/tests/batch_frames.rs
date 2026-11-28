//! The `batch` frame on the live streams of the real server (ADR-0021, slice 5): where it comes in the queue, what it
//! holds, who gets it, and what its headroom does and does not promise.

mod activity_world;
use activity_world::*;
use meldframe_host::Principal;
use meldframe_http_live::LiveSource;
use meldframe_protocol::ServerMsg;
use meldframe_server::Frame;
use serde_json::json;

type Out = Frame<meldframe_host::StoredOp, ServerMsg>;

fn who(ws: &str, actor: &str) -> Principal {
    Principal::new(ws, actor)
}

/// Everything queued for a stream: the numbers of the durable ops, the batch frames and the resync markers.
fn drain(w: &World, p: &Principal, file: &str, id: meldframe_server::StreamId) -> Vec<Out> {
    w.server.poll(p, file, id, 0, 100_000)
}

async fn one_op_calls(c: &Client, from: usize, to: usize) {
    for i in from..to {
        let r = call(
            c,
            "apply_commands",
            apply(
                "f",
                &format!("b{i}"),
                vec![frame(&format!("n{i}"), "N")],
                json!({}),
            ),
        )
        .await;
        assert!(!r.is_error, "{}", r.body);
    }
}

fn batches(frames: &[Out]) -> Vec<&ServerMsg> {
    fn hint(f: &Out) -> Option<&ServerMsg> {
        match f {
            Frame::Ephemeral(m @ ServerMsg::Batch { .. }) => Some(m),
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

#[tokio::test]
async fn the_frame_follows_the_ops_of_its_call_and_holds_the_header() {
    let w = start(true, false).await;
    let reader = who("ws_1", "user:bea");
    let id = w.server.subscribe(&reader, "f", 0, 0).unwrap().id;
    let stranger = w
        .server
        .subscribe(&who("ws_2", "agent:gamma"), "f", 0, 0)
        .unwrap()
        .id;
    let c = connect(&w.h, AGENT).await;
    let note = json!({"note": "Hero: new title"});
    call(
        &c,
        "apply_commands",
        apply("f", "k1", vec![frame("a", "A"), frame("b", "B")], note),
    )
    .await;
    let got = drain(&w, &reader, "f", id);
    let kinds: Vec<&str> = got
        .iter()
        .map(|f| match f {
            Frame::Durable { .. } => "op",
            Frame::Ephemeral(ServerMsg::Batch { .. }) => "batch",
            Frame::Ephemeral(_) => "touch",
            _ => "other",
        })
        .collect();
    // The ops, their touch hint when the server sends them, and the batch last.
    assert_eq!(kinds.iter().filter(|k| **k == "op").count(), 2);
    assert_eq!(kinds.last(), Some(&"batch"), "{kinds:?}");
    let ServerMsg::Batch {
        room,
        n,
        rev,
        actor,
        key_prefix,
        tool,
        commands,
        written,
        atomic,
        note,
        review,
        reviewed_by,
        unreviewed_count,
        ..
    } = batches(&got)[0].clone()
    else {
        unreachable!()
    };
    assert_eq!((room.as_str(), n, rev), ("f", 1, 1));
    assert_eq!(
        (actor.as_str(), key_prefix.as_str(), tool.as_str()),
        ("agent:alpha", "k1", "apply_commands")
    );
    assert_eq!(
        (commands, written, atomic, unreviewed_count),
        (2, 2, true, 1)
    );
    assert_eq!(
        (note.as_deref(), review.as_str(), reviewed_by),
        (Some("Hero: new title"), "unreviewed", None)
    );
    // Another workspace's stream of the same file name gets nothing.
    assert!(drain(&w, &who("ws_2", "agent:gamma"), "f", stranger).is_empty());
}

#[tokio::test]
async fn a_second_call_of_the_batch_that_changes_nothing_sends_no_frame() {
    let w = start(true, false).await;
    let reader = who("ws_1", "user:bea");
    let id = w.server.subscribe(&reader, "f", 0, 0).unwrap().id;
    let c = connect(&w.h, AGENT).await;
    let args = apply("f", "k1", vec![frame("a", "A")], json!({}));
    call(&c, "apply_commands", args.clone()).await;
    assert_eq!(batches(&drain(&w, &reader, "f", id)).len(), 1);
    // The replay of the same key writes nothing new, so the header is as it was.
    call(&c, "apply_commands", args).await;
    assert!(batches(&drain(&w, &reader, "f", id)).is_empty());
}

#[tokio::test]
async fn a_burst_below_the_boundary_never_pushes_a_durable_op_into_a_resync() {
    let w = start(true, false).await;
    let reader = who("ws_1", "user:bea");
    let id = w.server.subscribe(&reader, "f", 0, 0).unwrap().id;
    let c = connect(&w.h, AGENT).await;
    // 130 calls, each one op and one batch frame, into a queue of 256 that nobody reads: below the boundary of 136
    // (the next test). Without headroom the frames fill the queue by the 128th call and the 129th op is dropped for a
    // resync; with it the frames stop 16 slots short and every op is still queued.
    one_op_calls(&c, 0, 130).await;
    let got = drain(&w, &reader, "f", id);
    assert_eq!(resyncs(&got), 0, "a durable op was delayed into a resync");
    assert_eq!(durable(&got), 130);
    let frames = batches(&got).len();
    assert!(frames > 0 && frames < 130, "{frames} batch frames");
}

/// The headroom keeps the last 16 slots of the queue free of frames and does not cap their share: until the queue holds
/// 240 each one-op call adds an op and a frame (120 calls), then only ops (16 more fill the queue), so a stalled reader
/// is resynced by the 137th call and not by the 257th. The frames halve its slack and never cause a resync by themselves.
#[tokio::test]
async fn the_frames_halve_the_slack_of_a_stalled_reader() {
    for (calls, resync, ops) in [(136, 0, 136), (137, 1, 136)] {
        let w = start(true, false).await;
        let reader = who("ws_1", "user:bea");
        let id = w.server.subscribe(&reader, "f", 0, 0).unwrap().id;
        let c = connect(&w.h, AGENT).await;
        one_op_calls(&c, 0, calls).await;
        let got = drain(&w, &reader, "f", id);
        assert_eq!(
            (resyncs(&got), durable(&got)),
            (resync, ops),
            "{calls} calls"
        );
        assert_eq!(batches(&got).len(), 120, "{calls} calls");
    }
}

#[tokio::test]
async fn a_dropped_frame_gives_no_signal_and_the_list_route_heals_it() {
    let w = start(true, false).await;
    let reader = who("ws_1", "user:bea");
    let id = w.server.subscribe(&reader, "f", 0, 0).unwrap().id;
    let c = connect(&w.h, AGENT).await;
    one_op_calls(&c, 0, 130).await;
    let got = drain(&w, &reader, "f", id);
    let seen: Vec<u32> = batches(&got)
        .iter()
        .map(|m| match m {
            ServerMsg::Batch { n, .. } => *n,
            _ => 0,
        })
        .collect();
    let last = *seen.last().unwrap();
    assert!(last < 130, "some frame was dropped");
    // The frames that came are in order and whole up to the first drop: nothing tells the client about the rest.
    assert_eq!(seen, (1..=last).collect::<Vec<_>>());
    assert_eq!(resyncs(&got), 0);
    // The route has every batch the stream missed.
    let url = format!("http://{}/v1/files/f/batches?after={last}", w.h.addr);
    let r = reqwest::Client::new()
        .get(url)
        .bearer_auth(PERSON)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let ns: Vec<u64> = body["batches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["n"].as_u64().unwrap())
        .collect();
    assert_eq!(ns, (u64::from(last) + 1..=130).rev().collect::<Vec<_>>());
    assert_eq!(body["unreviewedCount"], 130);
}
