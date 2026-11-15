//! Accepted ops reach the subscribers of their file as durable frames (ADR-0009), in order and with their
//! sequence numbers; a reconnect gets exactly the missed ones, from the window or the store; a stalled
//! client resyncs; the caps, heartbeats and workspaces hold through the server. The queue arithmetic itself
//! is tested in `meldframe-stream`.

use meldframe_host::{Op, Principal, StoredOp};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::{Command, ServerMsg};
use meldframe_server::{Config, Frame, Server, StreamError, StreamId, SubscribeError};

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

fn who(ws: &str) -> Principal {
    Principal::new(ws, "agent:test")
}

fn add(n: u64) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": format!("n{n}"), "type": "frame"}))
        .unwrap();
    Op::new(format!("k{n}"), Command::AddNode { node, index: None })
}

fn write(s: &mut Server, ws: &str, from: u64, to: u64) {
    for n in from..=to {
        s.submit_tool("paint", "f", &who(ws), vec![add(n)]).unwrap();
    }
}

fn cfg(window: usize, queue: usize) -> Config {
    Config {
        window,
        queue,
        max_streams: 4,
        heartbeat_ms: 500,
        dead_after_ms: 1000,
        stall_ms: 5000,
    }
}

fn server(window: usize, queue: usize, total: usize) -> Server {
    Server::assemble_with(vec![Box::new(Painter)]).with_live(cfg(window, queue), total)
}

/// The numbers a client sees: the store replay, then the durable frames it polls.
fn got(s: &mut Server, ws: &str, id: StreamId, replay: &[StoredOp]) -> Vec<u64> {
    let mut out: Vec<u64> = replay.iter().map(|o| o.seq).collect();
    while let Some(f) = s.poll(&who(ws), "f", id, 0) {
        if let Frame::Durable { seq, data } = f {
            assert_eq!((data.seq, data.actor.as_str()), (seq, "agent:test"));
            out.push(seq);
        }
    }
    out
}

#[test]
fn subscribers_receive_accepted_ops_in_order_and_a_retry_or_refusal_publishes_nothing() {
    let mut s = server(8, 8, 8);
    let w = who("ws_1");
    let a = s.subscribe(&w, "f", 0, 0).unwrap();
    let b = s.subscribe(&w, "f", 0, 0).unwrap();
    s.submit_tool("paint", "f", &w, vec![add(1), add(2)])
        .unwrap();
    write(&mut s, "ws_1", 2, 3); // key k2 again: written false, only 3 is new
    assert!(s.submit_tool("nope", "f", &w, vec![add(9)]).is_err());
    assert_eq!(got(&mut s, "ws_1", a.id, &a.replay), [1, 2, 3]);
    assert_eq!(got(&mut s, "ws_1", b.id, &b.replay), [1, 2, 3]);
}

#[test]
fn a_reconnect_gets_the_store_range_then_the_window_and_nothing_twice() {
    let mut s = server(2, 8, 8);
    let w = who("ws_1");
    let keep = s.subscribe(&w, "f", 0, 0).unwrap();
    write(&mut s, "ws_1", 1, 5); // the window holds 4 and 5
    let b = s.subscribe(&w, "f", 1, 0).unwrap();
    assert_eq!(b.replay.iter().map(|o| o.seq).collect::<Vec<_>>(), [2, 3]);
    assert_eq!(got(&mut s, "ws_1", b.id, &b.replay), [2, 3, 4, 5]);
    s.unsubscribe(&w, "f", keep.id);
    s.unsubscribe(&w, "f", b.id);
    write(&mut s, "ws_1", 6, 7); // no hub now: a late client reads it all from the store
    let c = s.subscribe(&w, "f", 5, 0).unwrap();
    assert_eq!(got(&mut s, "ws_1", c.id, &c.replay), [6, 7]);
}

#[test]
fn a_stalled_client_resyncs_through_the_server_and_the_writer_never_waits() {
    let mut s = server(8, 3, 8);
    let w = who("ws_1");
    let stalled = s.subscribe(&w, "f", 0, 0).unwrap();
    let live = s.subscribe(&w, "f", 0, 0).unwrap();
    for n in 1..=6 {
        write(&mut s, "ws_1", n, n);
        assert_eq!(got(&mut s, "ws_1", live.id, &[]), [n]);
    }
    let frames: Vec<_> = std::iter::from_fn(|| s.poll(&w, "f", stalled.id, 0)).collect();
    assert!(matches!(
        frames.last(),
        Some(Frame::Resync {
            after_seq: 3,
            latest_seq: 6
        })
    ));
    let again = s.resubscribe(&w, "f", stalled.id, 3, 0).unwrap();
    assert_eq!(got(&mut s, "ws_1", stalled.id, &again.replay), [4, 5, 6]);
    assert_eq!(
        s.publish_ephemeral(&w, "f", Some(stalled.id), ServerMsg::Pong {}),
        1
    );
    assert!(matches!(
        s.poll(&w, "f", stalled.id, 0),
        Some(Frame::Ephemeral(_))
    ));
}

#[test]
fn the_cap_refuses_and_a_failed_subscribe_leaves_no_room_open_and_no_slot_taken() {
    let mut s = server(8, 8, 1);
    let w = who("ws_1");
    let a = s.subscribe(&w, "f", 0, 0).unwrap();
    let full = s.subscribe(&w, "g", 0, 0).unwrap_err();
    assert!(matches!(full, SubscribeError::Stream(StreamError::Full)));
    assert!(
        s.with_room("ws_1", "g", |_| ()).is_none(),
        "the failed call closed its room"
    );
    assert!(
        s.with_room("ws_1", "f", |_| ()).is_some(),
        "an open room stays open"
    );
    s.unsubscribe(&w, "f", a.id);
    s.subscribe(&w, "g", 0, 0).unwrap();
}

#[test]
fn heartbeats_queue_and_silent_streams_are_dropped_by_tick_while_seen_ones_stay() {
    let mut s = server(8, 8, 8);
    let w = who("ws_1");
    let quiet = s.subscribe(&w, "f", 0, 0).unwrap().id;
    let chatty = s.subscribe(&w, "f", 0, 0).unwrap().id;
    s.seen(&w, "f", chatty, 900);
    assert!(s.tick(500).is_empty());
    assert!(matches!(
        s.poll(&w, "f", chatty, 500),
        Some(Frame::Heartbeat)
    ));
    let dropped = s.tick(1000);
    assert_eq!(
        dropped.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        [quiet]
    );
    assert!(s.poll(&w, "f", quiet, 1000).is_none());
    assert_eq!(
        s.tick(1900).len(),
        1,
        "chatty is dropped once it goes quiet too"
    );
}

#[test]
fn a_workspace_cannot_read_close_or_publish_into_another_workspaces_stream() {
    let mut s = server(8, 8, 8);
    let (one, two) = (who("ws_1"), who("ws_2"));
    let b = s.subscribe(&two, "f", 0, 0).unwrap();
    write(&mut s, "ws_1", 1, 2); // the same file name, another file
    assert_eq!(got(&mut s, "ws_2", b.id, &[]), Vec::<u64>::new());
    write(&mut s, "ws_2", 1, 1);
    assert!(s.poll(&one, "f", b.id, 0).is_none(), "ws_1 cannot poll it");
    s.unsubscribe(&one, "f", b.id);
    assert_eq!(
        s.publish_ephemeral(&one, "f", Some(b.id), ServerMsg::Pong {}),
        0
    );
    assert_eq!(
        got(&mut s, "ws_2", b.id, &[]),
        [1],
        "it is still open and got its own file's op"
    );
}
