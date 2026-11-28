//! A stream belongs to the actor that opened it: another actor of the same workspace cannot poll, ping,
//! restart or close it. A failed subscribe closes the room only when the call opened it.

use meldframe_host::{Op, Principal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Config, Server, StreamError, SubscribeError};

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

fn who(actor: &str) -> Principal {
    Principal::new("ws_1", actor)
}

fn add(n: u64) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": format!("n{n}"), "type": "frame"}))
        .unwrap();
    Op::new(format!("k{n}"), Command::AddNode { node, index: None })
}

fn server(total: usize) -> Server {
    let config = Config {
        window: 8,
        queue: 8,
        max_streams: 4,
        heartbeat_ms: 500,
        dead_after_ms: 1000,
        stall_ms: 5000,
    };
    Server::assemble_with(vec![Box::new(Painter)]).with_live(config, total)
}

#[test]
fn another_actor_of_the_workspace_cannot_use_a_stream() {
    let mut s = server(8);
    let (owner, other) = (who("agent:a"), who("agent:b"));
    let sub = s.subscribe(&owner, "f", 0, 0).unwrap();
    s.submit_tool("paint", "f", &owner, vec![add(1)]).unwrap();
    assert!(s.poll(&other, "f", sub.id, 0).is_none(), "cannot poll it");
    assert!(matches!(
        s.resubscribe(&other, "f", sub.id, 0, 0),
        Err(SubscribeError::Stream(StreamError::Unknown))
    ));
    s.unsubscribe(&other, "f", sub.id);
    assert_eq!(s.live_stream_count(), 1, "cannot close it");
    // It is still the owner's, with its frame unread.
    assert!(s.poll(&owner, "f", sub.id, 0).is_some());
    s.unsubscribe(&owner, "f", sub.id);
    assert_eq!(s.live_stream_count(), 0);
}

#[test]
fn a_failed_subscribe_closes_the_room_only_when_the_call_opened_it() {
    let mut s = server(1);
    let w = who("agent:a");
    s.subscribe(&w, "f", 0, 0).unwrap();
    // "g" was opened by a write, not by the failing subscribe: it stays open.
    s.submit_tool("paint", "g", &w, vec![add(1)]).unwrap();
    let err = s.subscribe(&w, "g", 0, 0).unwrap_err();
    assert!(matches!(err, SubscribeError::Stream(StreamError::Full)));
    assert!(s.with_room("ws_1", "g", |_| ()).is_some());
    // "h" was opened by the failing subscribe: it is closed again.
    assert!(s.subscribe(&w, "h", 0, 0).is_err());
    assert!(s.with_room("ws_1", "h", |_| ()).is_none());
}

#[test]
fn a_bad_file_name_is_refused_by_subscribe() {
    let mut s = server(8);
    let err = s.subscribe(&who("agent:a"), "a\0b", 0, 0).unwrap_err();
    assert!(matches!(
        err,
        SubscribeError::Rooms(meldframe_rooms::RegistryError::InvalidName { .. })
    ));
    assert_eq!(s.live_stream_count(), 0);
}

#[test]
fn live_only_workspaces_get_idle_rooms_back_and_a_resubscribe_reopens_an_evicted_room() {
    let mut s = server(8).with_workspace_rooms(2).with_idle_after(30);
    let w = who("agent:a");
    let first = s.subscribe(&w, "f", 0, 0).unwrap();
    s.subscribe(&w, "g", 0, 0).unwrap();
    assert!(s.subscribe(&w, "h", 0, 0).is_err(), "at the room limit");
    std::thread::sleep(std::time::Duration::from_millis(80));
    s.subscribe(&w, "h", 0, 0).unwrap();
    assert!(
        s.with_room("ws_1", "f", |_| ()).is_none(),
        "f was idle and is closed"
    );
    s.resubscribe(&w, "f", first.id, 0, 0).unwrap();
    assert!(s.with_room("ws_1", "f", |_| ()).is_some());
}

#[test]
fn the_owner_map_follows_the_streams() {
    let mut s = server(2);
    let w = who("agent:a");
    let a = s.subscribe(&w, "f", 0, 0).unwrap();
    s.subscribe(&w, "g", 0, 0).unwrap();
    assert_eq!(s.live_owner_count(), 2);
    // A failed subscribe (the stream cap) never sets an owner.
    assert!(s.subscribe(&w, "h", 0, 0).is_err());
    assert_eq!(s.live_owner_count(), 2);
    s.unsubscribe(&w, "f", a.id);
    assert_eq!(s.live_owner_count(), 1);
    // The tick forgets the stream it drops as silent (dead_after is 1000).
    assert_eq!(s.tick(5000).len(), 1);
    assert_eq!(s.live_owner_count(), 0);
}

#[test]
fn the_same_actor_on_another_file_or_workspace_does_not_touch_a_stream() {
    let mut s = server(8);
    let w = who("agent:a");
    let elsewhere = Principal::new("ws_2", "agent:a");
    let sub = s.subscribe(&w, "f", 0, 0).unwrap();
    s.unsubscribe(&w, "g", sub.id);
    s.unsubscribe(&elsewhere, "f", sub.id);
    assert_eq!(s.live_stream_count(), 1);
    assert!(s.resubscribe(&elsewhere, "f", sub.id, 0, 0).is_err());
    assert!(s.resubscribe(&w, "g", sub.id, 0, 0).is_err());
    s.unsubscribe(&w, "f", sub.id);
    assert_eq!(s.live_stream_count(), 0);
}

#[test]
fn a_ping_from_a_non_owner_does_not_keep_a_stream_alive() {
    let mut s = server(8);
    let sub = s.subscribe(&who("agent:a"), "f", 0, 0).unwrap();
    s.seen(&who("agent:b"), "f", sub.id, 900);
    assert_eq!(
        s.tick(1500).len(),
        1,
        "dropped: only the owner's ping counts"
    );
}
