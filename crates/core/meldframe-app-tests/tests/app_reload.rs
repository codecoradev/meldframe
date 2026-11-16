//! Own ops are matched by the key the event carries, a halted app recovers from a snapshot instead of
//! being made again, and the pending ops of the replica are capped (issue 90).

mod common;

use common::*;
use meldframe_app::{Action, App, Connection, Effect, Event, Frame, OutOp};
use meldframe_protocol::{Command, NodePatch};
use serde_json::json;

fn live() -> App {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.take_events();
    app
}

fn mv(id: &str, x: f64) -> Action {
    Action::Move {
        id: id.into(),
        x,
        y: 0.0,
    }
}

fn ops(fx: &[Effect]) -> Vec<OutOp> {
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.clone(),
            _ => vec![],
        })
        .collect()
}

fn event(seq: u64, actor: &str, key: Option<&str>, command: Command) -> Action {
    Action::Frame {
        now_ms: 1,
        frame: Frame::Event {
            seq,
            actor: actor.into(),
            key: key.map(str::to_string),
            command,
        },
    }
}

fn set_x(id: &str, x: f64) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            x: Some(x),
            ..Default::default()
        },
    }
}

fn ghost() -> Command {
    set_x("ghost", 1.0)
}

/// A live app that sent one move, then received an event that cannot be applied: halted, one op pending.
fn halted() -> (App, OutOp) {
    let mut app = live();
    let sent = ops(&app.dispatch(mv("r1", 3.0))).remove(0);
    app.dispatch(event(1, "agent:a", None, ghost()));
    assert_eq!(app.state().connection, Connection::Halted);
    app.take_events();
    (app, sent)
}

fn reload(nodes: Vec<meldframe_protocol::Node>, seq: u64, applied: &[&str]) -> Action {
    Action::Reload {
        now_ms: 9,
        nodes,
        seq,
        applied: applied.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn an_event_of_my_actor_is_my_op_only_when_its_key_matches() {
    let mut app = live();
    let out = ops(&app.dispatch(mv("r1", 3.0))).remove(0);
    // Another session of the same actor made the same move: same actor, same command, another key.
    app.dispatch(event(1, "user:me", Some("other-1"), out.command.clone()));
    assert_eq!(
        app.state().pending,
        std::slice::from_ref(&out.key),
        "not my ack"
    );
    assert_eq!(app.state().confirmed_seq, 1);
    app.dispatch(event(2, "user:me", Some(&out.key), out.command));
    assert!(app.state().pending.is_empty());
    assert_eq!(app.state().confirmed_seq, 2);
}

#[test]
fn an_event_frame_without_a_key_still_reads_and_is_matched_by_command() {
    let json = json!({"type":"event","seq":1,"actor":"user:me",
        "command":{"type":"updateNode","id":"r1","patch":{"x":3.0,"y":0.0}}});
    let frame: Frame = serde_json::from_value(json).unwrap();
    assert!(matches!(&frame, Frame::Event { key: None, .. }));
    let mut app = live();
    app.dispatch(mv("r1", 3.0));
    app.dispatch(Action::Frame { now_ms: 1, frame });
    assert!(app.state().pending.is_empty());
    let keyed = serde_json::to_value(Frame::Event {
        seq: 1,
        actor: "a".into(),
        key: Some("k".into()),
        command: ghost(),
    })
    .unwrap();
    assert_eq!(keyed["key"], "k");
}

#[test]
fn a_halted_app_reloads_from_a_snapshot_and_subscribes_again() {
    let (mut app, sent) = halted();
    let mut snapshot = doc();
    snapshot.iter_mut().find(|n| n.id == "t1").unwrap().x = 40.0;
    let fx = app.dispatch(reload(snapshot, 4, &[]));
    assert_eq!(app.state().connection, Connection::Connecting);
    assert!(
        fx.iter()
            .any(|e| matches!(e, Effect::Subscribe { after_seq: 4 }))
    );
    assert!(
        fx.iter()
            .any(|e| matches!(e, Effect::Render { nodes } if nodes.is_empty()))
    );
    assert_eq!(get(&app, "t1").x, 40.0, "the snapshot");
    assert_eq!(get(&app, "r1").x, 3.0, "my pending move is replayed on it");
    assert_eq!(
        app.state().pending,
        std::slice::from_ref(&sent.key),
        "same key"
    );
    assert_eq!(app.state().confirmed_seq, 4);
    let events = app.take_events();
    assert!(events.contains(&Event::ConnectionChanged {
        connection: Connection::Connecting
    }));
    // Once subscribed the pending op goes out again, with its key.
    let fx = app.dispatch(Action::Connected { now_ms: 10 });
    assert_eq!(ops(&fx)[0].key, sent.key);
    assert_eq!(app.state().connection, Connection::Live);
    // And edits work again.
    assert!(!ops(&app.dispatch(mv("t1", 5.0))).is_empty());
}

#[test]
fn ops_the_snapshot_already_holds_are_not_sent_again() {
    let (mut app, sent) = halted();
    let mut snapshot = doc();
    snapshot.iter_mut().find(|n| n.id == "r1").unwrap().x = 3.0;
    app.dispatch(reload(snapshot, 4, &[&sent.key]));
    assert!(app.state().pending.is_empty());
    assert!(ops(&app.dispatch(Action::Connected { now_ms: 10 })).is_empty());
}

#[test]
fn a_bad_snapshot_is_refused_and_the_app_stays_halted() {
    let (mut app, sent) = halted();
    let mut bad = doc();
    bad.push(node("r1", "rect", Some("root")));
    let fx = app.dispatch(reload(bad, 4, &[]));
    assert!(fx.is_empty());
    assert_eq!(app.state().connection, Connection::Halted);
    assert_eq!(app.state().pending, [sent.key]);
    assert_eq!(
        app.take_events(),
        [Event::Refused {
            code: "E_DUPLICATE_ID".into()
        }]
    );
}

#[test]
fn a_document_action_beyond_the_cap_is_refused_and_changes_nothing() {
    let mut app = live();
    app.set_max_pending(2);
    assert_eq!(ops(&app.dispatch(mv("r1", 1.0))).len(), 1);
    assert_eq!(ops(&app.dispatch(mv("r1", 2.0))).len(), 1);
    app.take_events();
    let fx = app.dispatch(mv("r1", 3.0));
    assert!(ops(&fx).is_empty());
    assert_eq!(get(&app, "r1").x, 2.0);
    assert_eq!(app.state().pending.len(), 2);
    assert_eq!(
        app.take_events(),
        [Event::Refused {
            code: "outbox_full".into()
        }]
    );
    // Undo is refused the same way.
    assert!(ops(&app.dispatch(Action::Undo {})).is_empty());
    assert!(
        matches!(app.take_events().as_slice(), [Event::Refused { code }] if code == "outbox_full")
    );
}

#[test]
fn a_list_action_that_does_not_fit_applies_nothing() {
    let mut app = live();
    app.set_max_pending(2);
    app.dispatch(Action::Select {
        ids: ids(&["root", "r1", "t1"]),
    });
    app.take_events();
    let fx = app.dispatch(Action::MoveBy { dx: 5.0, dy: 0.0 });
    assert!(ops(&fx).is_empty());
    assert_eq!(
        (get(&app, "root").x, get(&app, "r1").x, get(&app, "t1").x),
        (0.0, 0.0, 0.0)
    );
    assert!(app.state().pending.is_empty());
    assert!(
        matches!(app.take_events().as_slice(), [Event::Refused { code }] if code == "outbox_full")
    );
}

fn acked(key: &str, seq: u64) -> Action {
    Action::Frame {
        now_ms: 2,
        frame: Frame::Acked {
            key: key.into(),
            seq,
        },
    }
}

#[test]
fn a_retryable_refusal_does_not_halt_the_app_when_a_later_op_is_confirmed_first() {
    let mut app = live();
    let a1 = ops(&app.dispatch(mv("r1", 1.0))).remove(0);
    let a2 = ops(&app.dispatch(mv("t1", 2.0))).remove(0);
    app.dispatch(Action::Frame {
        now_ms: 1,
        frame: Frame::Rejected {
            key: a1.key.clone(),
            code: "quota_exceeded".into(),
            retryable: true,
        },
    });
    app.take_events();
    app.dispatch(event(1, "user:me", Some(&a2.key), a2.command));
    assert_eq!(app.state().connection, Connection::Live);
    assert_eq!(app.state().pending, std::slice::from_ref(&a1.key));
    assert_eq!(app.state().confirmed_seq, 1);
    assert_eq!(get(&app, "r1").x, 1.0, "a1 is still in the view");
    app.dispatch(event(2, "user:me", Some(&a1.key), a1.command));
    assert!(app.state().pending.is_empty());
    assert_eq!(app.state().connection, Connection::Live);
}

#[test]
fn a_duplicate_ack_drains_a_pending_op_the_snapshot_already_holds() {
    let (mut app, sent) = halted();
    let mut snapshot = doc();
    snapshot.iter_mut().find(|n| n.id == "r1").unwrap().x = 3.0;
    app.dispatch(reload(snapshot, 4, &[]));
    let fx = app.dispatch(Action::Connected { now_ms: 10 });
    assert_eq!(ops(&fx)[0].key, sent.key, "sent again with its key");
    app.take_events();
    app.dispatch(acked(&sent.key, 9));
    assert_eq!(
        app.state().pending.len(),
        1,
        "an ack ahead of the stream waits"
    );
    app.dispatch(acked(&sent.key, 3));
    assert!(app.state().pending.is_empty());
    assert!(
        app.take_events()
            .contains(&Event::PendingChanged { count: 0 })
    );
    assert_eq!(get(&app, "r1").x, 3.0);
    let fx = app.dispatch(acked(&sent.key, 3));
    assert!(fx.is_empty());
}

#[test]
fn ops_that_no_longer_apply_after_a_reload_are_reported() {
    let (mut app, _) = halted();
    let mut snapshot = doc();
    snapshot.retain(|n| n.id != "r1");
    app.dispatch(reload(snapshot, 4, &[]));
    assert!(app.state().pending.is_empty());
    assert!(app.take_events().contains(&Event::OpsDropped { count: 1 }));
}

#[test]
fn a_snapshot_older_than_the_replica_is_refused_and_the_app_stays_halted() {
    let mut app = live();
    app.dispatch(event(1, "agent:a", None, set_t1()));
    app.dispatch(event(2, "agent:a", None, ghost()));
    assert_eq!(app.state().connection, Connection::Halted);
    app.take_events();
    app.dispatch(reload(doc(), 0, &[]));
    assert_eq!(app.state().connection, Connection::Halted);
    assert_eq!(
        app.take_events(),
        [Event::Refused {
            code: "E_SNAPSHOT_BEHIND".into()
        }]
    );
}

fn set_t1() -> Command {
    set_x("t1", 4.0)
}
