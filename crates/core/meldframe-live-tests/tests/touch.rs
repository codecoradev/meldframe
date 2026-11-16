//! Touch hints through the server: a write by any actor queues one ephemeral `touch` frame on the live
//! streams of that file (and no other file or workspace), a burst is coalesced per actor and file, a client
//! that never reads cannot grow the queue, and nothing is stored or replayed. The window arithmetic itself
//! is tested in `meldframe-stream`.

use meldframe_host::{Op, Principal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::{Command, NodePatch, ServerMsg};
use meldframe_server::{Config, Frame, Server, StreamId};
use std::time::Duration;

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

fn who(ws: &str, actor: &str) -> Principal {
    Principal::new(ws, actor)
}

fn add(n: u64) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": format!("n{n}"), "type": "frame"}))
        .unwrap();
    Op::new(format!("k{n}"), Command::AddNode { node, index: None })
}

fn cfg(queue: usize) -> Config {
    Config {
        window: 64,
        queue,
        max_streams: 8,
        heartbeat_ms: 60_000,
        dead_after_ms: 120_000,
        stall_ms: 600_000,
    }
}

fn server(queue: usize, touch_window_ms: Option<u64>) -> Server {
    let s = Server::assemble_with(vec![Box::new(Painter)]).with_live(cfg(queue), 8);
    match touch_window_ms {
        Some(ms) => s.with_touch_hints(ms),
        None => s,
    }
}

fn write(s: &mut Server, ws: &str, actor: &str, file: &str, ops: Vec<Op>) {
    s.submit_tool("paint", file, &who(ws, actor), ops).unwrap();
}

/// Everything queued for a stream: durable sequence numbers and the touch frames, in order.
fn drain(s: &mut Server, ws: &str, file: &str, id: StreamId) -> (Vec<u64>, Vec<ServerMsg>) {
    let (mut durable, mut touches) = (Vec::new(), Vec::new());
    while let Some(f) = s.poll(&who(ws, "agent:reader"), file, id, 0) {
        match f {
            Frame::Durable { seq, .. } => durable.push(seq),
            Frame::Ephemeral(m) => touches.push(m),
            _ => {}
        }
    }
    (durable, touches)
}

/// The stream got exactly these durable frames and no touch frame.
fn no_touch(got: (Vec<u64>, Vec<ServerMsg>), durable: &[u64]) {
    assert_eq!(got.0, durable);
    assert!(got.1.is_empty(), "unexpected touch frames: {:?}", got.1);
}

fn subscribe(s: &mut Server, ws: &str, file: &str) -> StreamId {
    s.subscribe(&who(ws, "agent:reader"), file, 0, 0)
        .unwrap()
        .id
}

fn touch(m: &ServerMsg) -> (&str, Vec<&str>, u64) {
    let ServerMsg::Touch {
        actor,
        node_ids,
        seq,
    } = m
    else {
        panic!("not a touch frame: {m:?}");
    };
    (actor, node_ids.iter().map(String::as_str).collect(), *seq)
}

#[test]
fn a_write_by_an_agent_is_one_touch_frame_for_its_file_and_none_for_another_file_or_workspace() {
    let mut s = server(8, Some(0));
    let same = subscribe(&mut s, "ws_1", "f");
    let other_file = subscribe(&mut s, "ws_1", "g");
    let other_ws = subscribe(&mut s, "ws_2", "f");
    write(&mut s, "ws_1", "agent:claude", "f", vec![add(1)]);

    let (durable, touches) = drain(&mut s, "ws_1", "f", same);
    assert_eq!(durable, [1]);
    assert_eq!(touches.len(), 1, "exactly one touch frame: {touches:?}");
    assert_eq!(touch(&touches[0]), ("agent:claude", vec!["n1"], 1));
    no_touch(drain(&mut s, "ws_1", "g", other_file), &[]);
    no_touch(drain(&mut s, "ws_2", "f", other_ws), &[]);
}

#[test]
fn a_person_touches_too_and_a_retry_or_refused_write_sends_nothing() {
    let mut s = server(8, Some(0));
    let id = subscribe(&mut s, "ws_1", "f");
    write(&mut s, "ws_1", "user:alice", "f", vec![add(1)]);
    write(&mut s, "ws_1", "user:alice", "f", vec![add(1)]); // the same key: nothing is written
    assert!(
        s.submit_tool("nope", "f", &who("ws_1", "user:alice"), vec![add(9)])
            .is_err()
    );
    let (_, touches) = drain(&mut s, "ws_1", "f", id);
    assert_eq!(touches.len(), 1);
    assert_eq!(touch(&touches[0]).0, "user:alice");
}

#[test]
fn a_batch_names_every_node_it_wrote_in_one_frame_with_the_last_sequence_number() {
    let mut s = server(8, Some(0));
    let id = subscribe(&mut s, "ws_1", "f");
    write(
        &mut s,
        "ws_1",
        "agent:claude",
        "f",
        vec![add(1), add(2), add(3)],
    );
    let (durable, touches) = drain(&mut s, "ws_1", "f", id);
    assert_eq!(durable, [1, 2, 3]);
    assert_eq!(touches.len(), 1);
    assert_eq!(
        touch(&touches[0]),
        ("agent:claude", vec!["n1", "n2", "n3"], 3)
    );
}

#[test]
fn a_burst_is_coalesced_and_the_latest_node_ids_win() {
    let window = 600;
    let mut s = server(64, Some(window));
    let id = subscribe(&mut s, "ws_1", "f");
    for n in 1..=10 {
        write(&mut s, "ws_1", "agent:claude", "f", vec![add(n)]);
    }
    // Another actor has a window of its own.
    write(&mut s, "ws_1", "agent:other", "f", vec![add(11)]);
    let (_, first) = drain(&mut s, "ws_1", "f", id);
    let sent: Vec<_> = first.iter().map(touch).collect();
    assert_eq!(
        sent,
        [
            ("agent:claude", vec!["n1"], 1),
            ("agent:other", vec!["n11"], 11)
        ],
        "the first touch of each actor goes at once, the other nine wait"
    );
    // Once the window is over the next look at the stream sends the latest ids, once.
    std::thread::sleep(Duration::from_millis(window + 100));
    let (_, later) = drain(&mut s, "ws_1", "f", id);
    let sent: Vec<_> = later.iter().map(touch).collect();
    assert_eq!(sent, [("agent:claude", vec!["n10"], 10)]);
    assert!(drain(&mut s, "ws_1", "f", id).1.is_empty());
}

#[test]
fn a_client_that_never_reads_holds_at_most_one_queue_of_frames() {
    let queue = 4;
    let mut s = server(queue, Some(0));
    let silent = subscribe(&mut s, "ws_1", "f");
    for n in 1..=200 {
        write(&mut s, "ws_1", "agent:claude", "f", vec![add(n)]);
    }
    let mut frames = 0;
    let mut marker = false;
    while let Some(f) = s.poll(&who("ws_1", "agent:reader"), "f", silent, 0) {
        marker |= matches!(f, Frame::Resync { .. });
        frames += 1;
    }
    assert!(
        frames <= queue + 1,
        "the queue and one resync marker, not {frames} frames"
    );
    assert!(
        marker,
        "the slow client is told to resync, as for any frame"
    );
}

#[test]
fn a_touch_is_never_stored_or_replayed_and_carries_no_content() {
    let mut s = server(8, Some(0));
    let early = subscribe(&mut s, "ws_1", "f");
    let patch = NodePatch {
        text: Some("secret copy".into()),
        ..NodePatch::default()
    };
    let node = serde_json::from_value(serde_json::json!({"id": "t1", "type": "text"})).unwrap();
    let ops = vec![
        Op::new("a", Command::AddNode { node, index: None }),
        Op::new(
            "b",
            Command::UpdateNode {
                id: "t1".into(),
                patch,
            },
        ),
    ];
    write(&mut s, "ws_1", "agent:claude", "f", ops);
    let (_, touches) = drain(&mut s, "ws_1", "f", early);
    let json = serde_json::to_value(&touches[0]).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"type": "touch", "actor": "agent:claude", "nodeIds": ["t1"], "seq": 2})
    );
    assert!(!json.to_string().contains("secret"));
    // The log holds the two ops and nothing else; a late client replays them and sees no touch frame.
    let late = s
        .subscribe(&who("ws_1", "agent:reader"), "f", 0, 0)
        .unwrap();
    no_touch(drain(&mut s, "ws_1", "f", late.id), &[1, 2]);
    assert_eq!(
        s.open("ws_1", "f").unwrap(),
        2,
        "no sequence number was spent"
    );
}

#[test]
fn touch_hints_are_off_until_the_server_turns_them_on() {
    let mut s = server(8, None);
    let id = subscribe(&mut s, "ws_1", "f");
    write(&mut s, "ws_1", "agent:claude", "f", vec![add(1)]);
    no_touch(drain(&mut s, "ws_1", "f", id), &[1]);
}
