//! A halted editor recovers against the real in-memory server: the shell loads the file again, the app
//! reloads, and the person's pending ops either come back confirmed or are sent again.

mod support;

use meldframe_app::{Action, Connection, Frame};
use meldframe_protocol::{Command, NodePatch};
use support::World;

fn set_x(id: &str, x: f64) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            x: Some(x),
            ..Default::default()
        },
    }
}

fn mv(id: &str, x: f64) -> Action {
    Action::Move {
        id: id.into(),
        x,
        y: 0.0,
    }
}

/// An event no file can take: the replica diverges and the app halts.
fn diverge(w: &mut World, i: usize) {
    let now = w.now;
    let seq = w.app(i).state().confirmed_seq + 1;
    w.act(
        i,
        Action::Frame {
            now_ms: now,
            frame: Frame::Event {
                seq,
                actor: "agent:x".into(),
                key: None,
                command: set_x("ghost", 1.0),
            },
        },
    );
    assert_eq!(w.app(i).state().connection, Connection::Halted);
}

/// The shell: drop the stream, load the file, tell the app which pending ops the file holds.
fn reload(w: &mut World, i: usize, applied: Vec<String>) {
    w.drop_link(i);
    let (nodes, seq) = w.room();
    let now = w.now;
    w.act(
        i,
        Action::Reload {
            now_ms: now,
            nodes,
            seq,
            applied,
        },
    );
}

#[test]
fn an_op_the_server_applied_before_the_snapshot_is_confirmed_by_the_reload() {
    let mut w = World::new(64);
    let a = w.join("user:a", "ca", 1);
    let b = w.join("user:b", "cb", 2);
    w.pump_all();
    w.act(a, mv("t1", 10.0));
    w.act(b, mv("root", 7.0));
    let mine = w.app(a).state().pending.clone();
    assert_eq!(mine.len(), 1, "sent, but its event has not come back");
    diverge(&mut w, a);
    reload(&mut w, a, mine);
    assert_eq!(w.app(a).state().connection, Connection::Live);
    w.pump_all();
    w.assert_converged();
}

#[test]
fn an_op_the_server_never_saw_is_sent_again_after_the_reload() {
    let mut w = World::new(64);
    let a = w.join("user:a", "ca", 1);
    let b = w.join("user:b", "cb", 2);
    w.pump_all();
    w.drop_link(a);
    w.act(a, mv("t1", 10.0));
    assert_eq!(
        w.app(a).state().pending.len(),
        1,
        "offline: queued, not sent"
    );
    w.act(b, mv("root", 7.0));
    w.pump_all();
    diverge(&mut w, a);
    reload(&mut w, a, vec![]);
    w.pump_all();
    w.assert_converged();
    assert_eq!(
        w.app(a).nodes().iter().find(|n| n.id == "t1").unwrap().x,
        10.0
    );
    assert_eq!(
        w.app(a).nodes().iter().find(|n| n.id == "root").unwrap().x,
        7.0
    );
}

#[test]
fn a_reloaded_editor_goes_on_editing_and_undoing_its_new_ops() {
    let mut w = World::new(64);
    let a = w.join("user:a", "ca", 1);
    w.pump_all();
    diverge(&mut w, a);
    reload(&mut w, a, vec![]);
    w.pump_all();
    w.act(a, mv("t1", 3.0));
    w.pump_all();
    w.act(a, Action::Undo {});
    w.pump_all();
    w.assert_converged();
    assert_eq!(
        w.app(a).nodes().iter().find(|n| n.id == "t1").unwrap().x,
        0.0
    );
}

#[test]
fn an_applied_op_left_out_of_applied_is_drained_by_its_duplicate_ack() {
    let mut w = World::new(64);
    let a = w.join("user:a", "ca", 1);
    w.pump_all();
    w.act(a, mv("t1", 10.0));
    diverge(&mut w, a);
    // The shell does not know which pending ops the file holds: it names none and the server tells it.
    reload(&mut w, a, vec![]);
    w.pump_all();
    w.assert_converged();
    assert!(w.app(a).state().pending.is_empty());
}
