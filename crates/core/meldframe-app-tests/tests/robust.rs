//! Bad input, retries, a diverged file, gaps and render lists.

mod common;
use common::*;
use meldframe_app::{Action, App, Connection, CreateKind, Effect, Event, Frame};
use meldframe_protocol::{Command, NodePatch};

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

fn frame(now_ms: u64, frame: Frame) -> Action {
    Action::Frame { now_ms, frame }
}

fn ev(seq: u64, actor: &str, command: Command) -> Action {
    frame(
        0,
        Frame::Event {
            seq,
            actor: actor.into(),
            key: None,
            command,
        },
    )
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

fn rejected(key: &str, retryable: bool, now_ms: u64) -> Action {
    frame(
        now_ms,
        Frame::Rejected {
            key: key.into(),
            code: "quota_exceeded".into(),
            retryable,
        },
    )
}

fn op_keys(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.key.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

fn subscribes(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Subscribe { after_seq } => Some(*after_seq),
            _ => None,
        })
        .collect()
}

fn renders(fx: &[Effect]) -> Vec<Vec<String>> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Render { nodes } => Some(nodes.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn creating_at_a_point_that_is_not_a_number_is_refused_and_does_not_panic() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut app = live();
        let fx = app.dispatch(Action::create_node(
            CreateKind::Frame,
            bad,
            0.0,
            Some("root".into()),
        ));
        assert!(fx.is_empty() && app.nodes().len() == 3);
        assert_eq!(
            app.take_events(),
            vec![Event::Refused {
                code: "invalid".into()
            }]
        );
    }
}

#[test]
fn two_apps_of_one_actor_and_prefix_make_disjoint_keys_and_ids() {
    let mut a = App::new("user:me", "c0", 1, doc(), 0).unwrap();
    let mut b = App::new("user:me", "c0", 2, doc(), 0).unwrap();
    let mut keys = vec![];
    let mut ids = vec![];
    for app in [&mut a, &mut b] {
        app.dispatch(Action::Connected { now_ms: 0 });
        for x in 1..=3 {
            let fx = app.dispatch(mv("r1", f64::from(x)));
            keys.extend(op_keys(&fx));
        }
        app.dispatch(Action::create_node(CreateKind::Text, 0.0, 0.0, None));
        ids.push(app.state().selection[0].clone());
    }
    assert_eq!(keys.len(), 6);
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), 6, "no key is used twice");
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn a_retryable_refusal_keeps_the_op_and_sends_it_again_with_the_same_key() {
    let mut app = live();
    let fx = app.dispatch(mv("r1", 3.0));
    let key = op_keys(&fx).remove(0);
    let sent_cmd = sent(&fx).remove(0);
    let fx = app.dispatch(rejected(&key, true, 100));
    assert!(op_keys(&fx).is_empty());
    assert!(
        fx.iter()
            .any(|e| matches!(e, Effect::WakeAfter { after_ms } if *after_ms > 0))
    );
    assert!(app.take_events().contains(&Event::OpRetry {
        key: key.clone(),
        attempt: 1
    }));
    assert_eq!(get(&app, "r1").x, 3.0, "the view keeps the edit");
    assert_eq!(app.state().pending, vec![key.clone()]);
    assert!(
        app.dispatch(Action::Tick { now_ms: 100 }).is_empty(),
        "not due yet"
    );
    let fx = app.dispatch(Action::Tick { now_ms: 5_000 });
    assert_eq!(op_keys(&fx), vec![key.clone()]);
    app.dispatch(ev(1, "user:me", sent_cmd));
    assert!(app.state().pending.is_empty());
    assert_eq!(app.state().confirmed_seq, 1);
}

#[test]
fn a_retryable_refusal_gives_up_after_a_few_tries_and_rolls_back() {
    let mut app = live();
    let key = op_keys(&app.dispatch(mv("r1", 3.0))).remove(0);
    let mut now = 0;
    for attempt in 1..=3u32 {
        app.dispatch(rejected(&key, true, now));
        assert!(app.take_events().contains(&Event::OpRetry {
            key: key.clone(),
            attempt
        }));
        now += 10_000;
        assert_eq!(
            op_keys(&app.dispatch(Action::Tick { now_ms: now })),
            vec![key.clone()]
        );
    }
    app.dispatch(rejected(&key, true, now));
    assert!(app.take_events().contains(&Event::OpRejected {
        key: key.clone(),
        code: "quota_exceeded".into(),
        retryable: true
    }));
    assert_eq!(get(&app, "r1").x, 0.0);
    assert!(app.state().pending.is_empty());
}

#[test]
fn a_final_refusal_rolls_back_at_once() {
    let mut app = live();
    let key = op_keys(&app.dispatch(mv("r1", 3.0))).remove(0);
    let fx = app.dispatch(rejected(&key, false, 0));
    assert!(!fx.iter().any(|e| matches!(e, Effect::WakeAfter { .. })));
    assert_eq!(get(&app, "r1").x, 0.0);
}

#[test]
fn a_diverged_file_halts_the_app_until_a_new_one_is_made() {
    let mut app = live();
    app.dispatch(ev(1, "agent:a", set_x("ghost", 1.0)));
    assert_eq!(app.state().connection, Connection::Halted);
    let events = app.take_events();
    assert_eq!(
        events.iter().filter(|e| **e == Event::Diverged {}).count(),
        1
    );
    assert!(app.dispatch(mv("r1", 5.0)).is_empty());
    assert_eq!(get(&app, "r1").x, 0.0);
    assert!(
        app.dispatch(ev(2, "agent:a", set_x("ghost", 2.0)))
            .is_empty()
    );
    assert!(app.dispatch(Action::Connected { now_ms: 9 }).is_empty());
    assert!(app.dispatch(Action::Reconnect { now_ms: 9 }).is_empty());
    assert!(app.take_events().is_empty(), "Diverged is raised once");
    app.dispatch(Action::SetTool {
        tool: meldframe_app::Tool::Text,
    });
    assert_eq!(
        app.state().tool,
        meldframe_app::Tool::Text,
        "the UI still works"
    );
}

#[test]
fn three_frames_ahead_of_the_numbers_give_one_subscribe_until_one_applies() {
    let mut app = live();
    app.dispatch(ev(1, "agent:a", set_x("r1", 1.0)));
    let mut all = vec![];
    for seq in [3, 4, 5] {
        all.extend(subscribes(&app.dispatch(ev(
            seq,
            "agent:a",
            set_x("r1", 9.0),
        ))));
    }
    assert_eq!(all, [1]);
    app.dispatch(ev(2, "agent:a", set_x("r1", 2.0)));
    assert_eq!(
        subscribes(&app.dispatch(ev(9, "agent:a", set_x("r1", 9.0)))),
        [2]
    );
}

#[test]
fn a_resync_marker_ahead_of_the_replica_resubscribes_from_the_replica() {
    let mut app = live();
    app.dispatch(ev(1, "agent:a", set_x("r1", 1.0)));
    let fx = app.dispatch(frame(
        0,
        Frame::Resync {
            after_seq: 9,
            latest_seq: 12,
        },
    ));
    assert_eq!(subscribes(&fx), [1]);
}

#[test]
fn removing_a_frame_asks_for_a_full_redraw_and_clears_state_of_the_subtree() {
    let mut app = live();
    app.dispatch(Action::ToggleCollapsed { id: "root".into() });
    app.dispatch(Action::ToggleCollapsed { id: "ghost".into() });
    assert_eq!(
        app.state().panels.collapsed,
        ids(&["root"]),
        "an unknown layer is ignored"
    );
    let fx = app.dispatch(ev(1, "agent:a", Command::RemoveNode { id: "root".into() }));
    assert_eq!(renders(&fx), vec![Vec::<String>::new()]);
    assert!(app.state().panels.collapsed.is_empty());
    let mut l = Loopback::live();
    let fx = l.send(Action::Delete {
        ids: ids(&["root"]),
    });
    assert_eq!(renders(&fx), vec![Vec::<String>::new()]);
}

#[test]
fn a_bad_selection_move_or_delete_applies_nothing() {
    let mut app = live();
    app.dispatch(Action::Select {
        ids: ids(&["r1", "t1"]),
    });
    app.take_events();
    assert!(
        app.dispatch(Action::MoveBy {
            dx: f64::NAN,
            dy: 0.0
        })
        .is_empty()
    );
    assert_eq!((get(&app, "r1").x, get(&app, "t1").x), (0.0, 0.0));
    assert_eq!(
        app.take_events(),
        vec![Event::Refused {
            code: "invalid".into()
        }]
    );
    assert!(
        app.dispatch(Action::Delete {
            ids: ids(&["r1", "ghost"])
        })
        .is_empty()
    );
    assert_eq!(app.nodes().len(), 3);
    assert_eq!(
        app.take_events(),
        vec![Event::Refused {
            code: "not_found".into()
        }]
    );
    assert!(app.state().pending.is_empty() && !app.state().history.can_undo);
}

#[test]
fn an_undo_that_removes_the_selected_node_clears_the_selection() {
    let mut l = Loopback::live();
    l.send(Action::create_node(CreateKind::Rectangle, 0.0, 0.0, None));
    assert_eq!(l.app.state().selection.len(), 1);
    l.send(Action::Undo {});
    assert!(l.app.state().selection.is_empty() && l.app.state().properties.is_none());
}
