//! Sessions that stress the connection and the server: heartbeats, ops in flight, refusals, concurrent
//! edits, a restart. Real server, two editors and an agent.

mod support;
use meldframe_app::{Action, CreateKind, Event};
use meldframe_protocol::Command;
use support::*;

fn create(kind: CreateKind, x: f64, y: f64, parent: &str) -> Action {
    Action::CreateNode {
        kind,
        x,
        y,
        parent: Some(parent.into()),
    }
}

fn selected(w: &World, i: usize) -> String {
    w.app(i).state().selection[0].clone()
}

fn node_of(w: &World, i: usize, id: &str) -> meldframe_protocol::Node {
    w.app(i)
        .nodes()
        .iter()
        .find(|n| n.id == id)
        .cloned()
        .unwrap()
}

/// Alice and Bob open the file. Returns their indexes.
fn two_editors(w: &mut World) -> (usize, usize) {
    let a = w.join("user:alice", "a", 1);
    let b = w.join("user:bob", "b", 1);
    assert_eq!(w.app(a).state().connection, meldframe_app::Connection::Live);
    (a, b)
}

#[test]
fn heartbeats_keep_a_quiet_connection_alive_past_the_silence_limit() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.clients[a].events.clear();
    // Two minutes of nothing but heartbeats. Alice hears them; Bob's shell stops pumping.
    for _ in 0..24 {
        w.now += 5_000;
        w.server.tick(w.now);
        assert!(w.pump(a) >= 1, "a heartbeat frame arrived");
        w.act(a, Action::Tick { now_ms: w.now });
        w.act(b, Action::Tick { now_ms: w.now });
    }
    assert!(
        w.clients[a].events.is_empty(),
        "no document or state change"
    );
    assert_eq!(w.app(a).state().connection, meldframe_app::Connection::Live);
    assert_eq!(
        w.app(b).state().connection,
        meldframe_app::Connection::Offline,
        "silent for over 45 s"
    );
}

#[test]
fn an_op_the_server_took_just_before_the_drop_is_confirmed_once_and_not_written_twice() {
    let mut w = World::new(64);
    let (a, _b) = two_editors(&mut w);
    let seq_before = w.room().1;
    w.act(
        a,
        Action::EditText {
            id: "t1".into(),
            text: "in flight".into(),
        },
    );
    assert_eq!(w.room().1, seq_before + 1, "the server has it");
    assert_eq!(w.app(a).state().pending.len(), 1, "but alice has not heard");
    w.drop_link(a);
    w.now += 1_000;
    w.act(a, Action::Reconnect { now_ms: w.now });
    w.pump_all();
    assert_eq!(w.room().1, seq_before + 1, "the resent op was a duplicate");
    assert_eq!(node_of(&w, a, "t1").text, "in flight");
    w.assert_converged();
}

#[test]
fn the_server_refuses_an_edit_of_a_node_that_is_gone_and_the_view_goes_back() {
    let mut w = World::new(64);
    let (a, _b) = two_editors(&mut w);
    w.agent(vec![Command::RemoveNode { id: "t1".into() }]);
    w.act(
        a,
        Action::EditText {
            id: "t1".into(),
            text: "too late".into(),
        },
    );
    assert!(
        w.clients[a]
            .events
            .iter()
            .any(|e| matches!(e, Event::OpRejected { code, .. } if code == "E_NODE_NOT_FOUND"))
    );
    assert!(w.app(a).state().pending.is_empty());
    w.pump_all();
    assert!(w.app(a).nodes().iter().all(|n| n.id != "t1"));
    w.assert_converged();
}

#[test]
fn two_editors_changing_the_same_node_at_once_converge() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.act(
        a,
        Action::EditText {
            id: "t1".into(),
            text: "alice".into(),
        },
    );
    w.act(
        b,
        Action::EditText {
            id: "t1".into(),
            text: "bob".into(),
        },
    );
    w.act(
        a,
        Action::Move {
            id: "t1".into(),
            x: 7.0,
            y: 7.0,
        },
    );
    w.pump_all();
    w.pump_all();
    assert_eq!(node_of(&w, a, "t1").text, node_of(&w, b, "t1").text);
    assert_eq!(node_of(&w, b, "t1").x, 7.0);
    w.assert_converged();
}

#[test]
fn an_offline_edit_of_a_node_deleted_meanwhile_is_dropped_and_everyone_converges() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.drop_link(a);
    w.act(
        a,
        Action::EditText {
            id: "t1".into(),
            text: "offline".into(),
        },
    );
    w.act(
        b,
        Action::Delete {
            ids: vec!["t1".into()],
        },
    );
    w.pump(b);
    w.now += 1_000;
    w.act(a, Action::Reconnect { now_ms: w.now });
    w.pump_all();
    assert!(w.app(a).nodes().iter().all(|n| n.id != "t1"));
    w.assert_converged();
}

#[test]
fn a_restarted_editor_with_the_same_actor_and_prefix_loses_no_edit() {
    let mut w = World::new(64);
    let a = w.join("user:alice", "a", 1);
    w.act(
        a,
        Action::EditText {
            id: "t1".into(),
            text: "first".into(),
        },
    );
    w.act(
        a,
        Action::Move {
            id: "t1".into(),
            x: 1.0,
            y: 1.0,
        },
    );
    w.pump_all();
    let again = w.join("user:alice", "a", 2);
    w.act(
        again,
        Action::EditText {
            id: "t1".into(),
            text: "second".into(),
        },
    );
    w.act(
        again,
        Action::Move {
            id: "t1".into(),
            x: 2.0,
            y: 2.0,
        },
    );
    w.pump_all();
    assert_eq!(node_of(&w, again, "t1").text, "second");
    assert!(
        w.clients[again]
            .events
            .iter()
            .all(|e| !matches!(e, Event::OpRejected { .. }))
    );
    assert_eq!(node_of(&w, a, "t1").x, 2.0);
    w.assert_converged();
}

#[test]
fn undoing_the_creation_of_the_selected_node_clears_the_selection_everywhere() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.act(a, create(CreateKind::Rectangle, 0.0, 0.0, "root"));
    let rect = selected(&w, a);
    w.pump_all();
    w.act(
        b,
        Action::Select {
            ids: vec![rect.clone()],
        },
    );
    w.act(a, Action::Undo {});
    w.pump_all();
    assert!(w.app(a).state().selection.is_empty());
    assert!(
        w.app(b).state().selection.is_empty(),
        "bob had it selected too"
    );
    w.assert_converged();
}
