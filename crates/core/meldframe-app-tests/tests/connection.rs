//! The connection state machine: connecting, live, dropped, backoff, reconnect, resync and heartbeats.
//! Frames are scripted; the real server comes in `meldframe-app-tests`.

use meldframe_app::{Action, App, Connection, Effect, Event, Frame};
use meldframe_protocol::{Command, Node, NodePatch};

fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

fn app() -> App {
    let nodes = vec![
        node("root", "frame", None),
        node("r1", "rect", Some("root")),
        node("t1", "text", Some("root")),
    ];
    App::new("user:me", "c0", 1, nodes, 0).unwrap()
}

fn get(app: &App, id: &str) -> Node {
    app.nodes().iter().find(|n| n.id == id).cloned().unwrap()
}

fn sent(effects: &[Effect]) -> Vec<Command> {
    effects
        .iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.command.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

fn subscribe_after(effects: &[Effect]) -> Vec<u64> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Subscribe { after_seq } => Some(*after_seq),
            _ => None,
        })
        .collect()
}

fn reconnect_after(effects: &[Effect]) -> Vec<u64> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Reconnect { after_ms } => Some(*after_ms),
            _ => None,
        })
        .collect()
}

fn move_to(id: &str, x: f64) -> Action {
    Action::Move {
        id: id.into(),
        x,
        y: 0.0,
    }
}

fn event(seq: u64, actor: &str, id: &str, x: f64) -> Action {
    Action::Frame {
        now_ms: 0,
        frame: Frame::Event {
            seq,
            actor: actor.into(),
            command: Command::UpdateNode {
                id: id.into(),
                patch: NodePatch {
                    x: Some(x),
                    ..Default::default()
                },
            },
        },
    }
}

#[test]
fn start_subscribes_from_the_last_confirmed_number() {
    let mut app = app();
    let fx = app.dispatch(Action::Start { now_ms: 0 });
    assert_eq!(subscribe_after(&fx), [0]);
    assert_eq!(app.state().connection, Connection::Connecting);
}

#[test]
fn ops_made_before_the_connection_is_live_are_held_and_sent_when_it_is() {
    let mut app = app();
    app.dispatch(Action::Start { now_ms: 0 });
    let fx = app.dispatch(move_to("r1", 4.0));
    assert!(sent(&fx).is_empty(), "nothing goes out while connecting");
    assert_eq!(app.state().pending.len(), 1);
    let fx = app.dispatch(Action::Connected { now_ms: 10 });
    assert_eq!(app.state().connection, Connection::Live);
    assert_eq!(sent(&fx).len(), 1);
}

#[test]
fn a_dropped_connection_goes_offline_and_asks_for_a_retry_with_growing_backoff() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    let fx = app.dispatch(Action::Disconnected {});
    assert_eq!(app.state().connection, Connection::Offline);
    let mut waits = reconnect_after(&fx);
    for _ in 0..7 {
        app.dispatch(Action::Reconnect { now_ms: 0 });
        waits.extend(reconnect_after(&app.dispatch(Action::Disconnected {})));
    }
    assert_eq!(
        waits,
        [1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000, 30_000]
    );
    assert!(
        app.dispatch(Action::Disconnected {}).is_empty(),
        "already offline"
    );
}

#[test]
fn reconnect_resubscribes_from_the_confirmed_number_and_connected_resets_the_backoff() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(event(1, "agent:a", "r1", 1.0));
    app.dispatch(event(2, "agent:a", "r1", 2.0));
    app.dispatch(Action::Disconnected {});
    app.take_events();
    let fx = app.dispatch(Action::Reconnect { now_ms: 1_000 });
    assert_eq!(
        app.state().connection,
        Connection::Reconnecting { attempt: 1 }
    );
    assert_eq!(subscribe_after(&fx), [2]);
    app.dispatch(Action::Connected { now_ms: 1_100 });
    assert_eq!(app.state().connection, Connection::Live);
    app.dispatch(Action::Tick { now_ms: 12_000 }); // live for over ten seconds: the backoff starts again
    let ev = app.take_events();
    assert!(ev.contains(&Event::ConnectionChanged {
        connection: Connection::Reconnecting { attempt: 1 }
    }));
    let fx = app.dispatch(Action::Disconnected {});
    assert_eq!(reconnect_after(&fx), [1_000]);
}

#[test]
fn a_reconnect_that_is_not_offline_is_ignored() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    assert!(app.dispatch(Action::Reconnect { now_ms: 5 }).is_empty());
    assert_eq!(app.state().connection, Connection::Live);
}

#[test]
fn edits_made_offline_stay_pending_and_go_out_again_with_the_same_keys() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    let first = app.dispatch(move_to("r1", 1.0));
    let key = match &first[1] {
        Effect::SendOps { ops } => ops[0].key.clone(),
        other => panic!("{other:?}"),
    };
    app.dispatch(Action::Disconnected {});
    let fx = app.dispatch(move_to("t1", 2.0));
    assert!(sent(&fx).is_empty());
    assert_eq!(app.state().pending.len(), 2);
    assert_eq!(get(&app, "t1").x, 2.0, "still shown at once");
    app.dispatch(Action::Reconnect { now_ms: 1_000 });
    let fx = app.dispatch(Action::Connected { now_ms: 1_001 });
    let Some(Effect::SendOps { ops }) = fx.iter().find(|e| matches!(e, Effect::SendOps { .. }))
    else {
        panic!("the unconfirmed ops are sent again")
    };
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].key, key);
}

#[test]
fn missed_frames_apply_in_order_and_a_repeat_changes_nothing() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(Action::Disconnected {});
    app.dispatch(Action::Reconnect { now_ms: 1_000 });
    app.dispatch(Action::Connected { now_ms: 1_001 });
    app.dispatch(event(1, "agent:a", "r1", 1.0));
    app.dispatch(event(2, "agent:a", "t1", 2.0));
    app.dispatch(event(2, "agent:a", "t1", 2.0));
    assert_eq!((get(&app, "r1").x, get(&app, "t1").x), (1.0, 2.0));
    assert_eq!(app.state().confirmed_seq, 2);
}

#[test]
fn a_resync_marker_resubscribes_from_the_marker() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(event(1, "agent:a", "r1", 1.0));
    let fx = app.dispatch(Action::Frame {
        now_ms: 5,
        frame: Frame::Resync {
            after_seq: 1,
            latest_seq: 9,
        },
    });
    assert_eq!(subscribe_after(&fx), [1]);
    assert_eq!(app.state().connection, Connection::Live);
}

#[test]
fn a_gap_in_the_numbers_resubscribes_from_the_last_applied() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(event(1, "agent:a", "r1", 1.0));
    let fx = app.dispatch(event(3, "agent:a", "r1", 3.0));
    assert_eq!(subscribe_after(&fx), [1]);
    assert_eq!(get(&app, "r1").x, 1.0);
}

#[test]
fn a_silent_live_connection_is_dropped_but_a_heartbeat_keeps_it() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    assert!(app.dispatch(Action::Tick { now_ms: 44_000 }).is_empty());
    app.dispatch(Action::Frame {
        now_ms: 44_000,
        frame: Frame::Heartbeat {},
    });
    assert!(app.dispatch(Action::Tick { now_ms: 80_000 }).is_empty());
    let fx = app.dispatch(Action::Tick { now_ms: 90_000 });
    assert_eq!(app.state().connection, Connection::Offline);
    assert_eq!(reconnect_after(&fx), [1_000]);
}

#[test]
fn an_attempt_that_never_connects_times_out_and_backs_off_longer() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(Action::Disconnected {});
    app.dispatch(Action::Reconnect { now_ms: 1_000 });
    assert!(app.dispatch(Action::Tick { now_ms: 5_000 }).is_empty());
    let fx = app.dispatch(Action::Tick { now_ms: 12_000 });
    assert_eq!(app.state().connection, Connection::Offline);
    assert_eq!(reconnect_after(&fx), [2_000]);
}

#[test]
fn the_heartbeat_changes_no_document_state() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.take_events();
    let before = app.state().clone();
    let fx = app.dispatch(Action::Frame {
        now_ms: 3,
        frame: Frame::Heartbeat {},
    });
    assert!(fx.is_empty() && app.take_events().is_empty());
    assert_eq!(*app.state(), before);
}

#[test]
fn a_connection_that_drops_soon_after_connecting_keeps_the_backoff() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(Action::Disconnected {});
    app.dispatch(Action::Reconnect { now_ms: 1_000 });
    app.dispatch(Action::Connected { now_ms: 1_100 });
    app.dispatch(Action::Tick { now_ms: 3_000 });
    let fx = app.dispatch(Action::Disconnected {});
    assert_eq!(
        reconnect_after(&fx),
        [2_000],
        "it was live for less than ten seconds"
    );
}

#[test]
fn a_first_connection_that_never_comes_up_times_out() {
    let mut app = app();
    app.dispatch(Action::Start { now_ms: 100 });
    assert!(app.dispatch(Action::Tick { now_ms: 5_000 }).is_empty());
    let fx = app.dispatch(Action::Tick { now_ms: 10_200 });
    assert_eq!(app.state().connection, Connection::Offline);
    assert_eq!(reconnect_after(&fx), [1_000]);
}

#[test]
fn a_clock_that_goes_backwards_drops_nothing() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 50_000 });
    assert!(app.dispatch(Action::Tick { now_ms: 10 }).is_empty());
    assert_eq!(app.state().connection, Connection::Live);
}
