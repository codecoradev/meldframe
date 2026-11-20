//! Document actions: each becomes commands in the replica, applied at once and sent when live.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, CreateKind, Effect, Event, Frame, Property, Tool};
use meldframe_protocol::{Command, NodeKind};

fn ops_of(eff: &[Effect]) -> Vec<meldframe_app::OutOp> {
    eff.iter()
        .find_map(|e| match e {
            Effect::SendOps { ops } => Some(ops.clone()),
            _ => None,
        })
        .expect("a SendOps effect")
}

fn create(kind: CreateKind, x: f64, y: f64) -> Action {
    Action::CreateNode {
        kind,
        x,
        y,
        parent: Some("root".into()),
        w: None,
        h: None,
    }
}

#[test]
fn create_makes_the_node_selects_it_and_returns_to_the_select_tool() {
    let mut l = Loopback::live();
    l.app.dispatch(Action::SetTool {
        tool: Tool::Rectangle,
    });
    let eff = l.send(create(CreateKind::Rectangle, 10.0, 20.0));
    let id = l.app.state().selection[0].clone();
    let n = get(&l.app, &id);
    assert_eq!((n.kind, n.x, n.y), (NodeKind::Rect, 10.0, 20.0));
    assert_eq!(n.parent_id.as_deref(), Some("root"));
    assert_eq!(l.app.state().tool, Tool::Select);
    assert!(matches!(sent(&eff).as_slice(), [Command::AddNode { .. }]));
    assert!(eff.iter().any(|e| matches!(e, Effect::Render { .. })));
    assert!(l.app.state().history.can_undo);
}

#[test]
fn a_frame_a_rectangle_and_a_text_node_can_be_created() {
    let mut l = Loopback::live();
    for (k, kind) in [
        (CreateKind::Frame, NodeKind::Frame),
        (CreateKind::Rectangle, NodeKind::Rect),
        (CreateKind::Text, NodeKind::Text),
    ] {
        l.send(create(k, 0.0, 0.0));
        let id = l.app.state().selection[0].clone();
        assert_eq!(get(&l.app, &id).kind, kind);
    }
    assert_eq!(l.app.nodes().len(), 6);
}

#[test]
fn move_resize_text_and_properties_update_the_node_and_the_panel() {
    let mut l = Loopback::live();
    l.send(Action::Select { ids: ids(&["r1"]) });
    l.send(Action::Move {
        id: "r1".into(),
        x: 5.0,
        y: 6.0,
    });
    l.send(Action::Resize {
        id: "r1".into(),
        w: 30.0,
        h: 40.0,
        x: None,
        y: None,
    });
    l.send(Action::SetProperty {
        id: "r1".into(),
        property: Property::Fill("#ff0000".into()),
    });
    l.send(Action::SetProperty {
        id: "r1".into(),
        property: Property::Name("Box".into()),
    });
    l.send(Action::EditText {
        id: "t1".into(),
        text: "Hello".into(),
    });
    let r = get(&l.app, "r1");
    assert_eq!((r.x, r.y, r.w, r.h), (5.0, 6.0, 30.0, 40.0));
    assert_eq!(
        (r.fill.lit().unwrap().as_str(), r.name.as_str()),
        ("#ff0000", "Box")
    );
    assert_eq!(get(&l.app, "t1").text, "Hello");
    let p = l.app.state().properties.clone().unwrap();
    assert_eq!(
        (p.w, p.fill.as_str(), p.name.as_str()),
        (30.0, "#ff0000", "Box")
    );
}

#[test]
fn moving_a_selection_by_a_delta_is_one_undo_step() {
    let mut l = Loopback::live();
    l.send(Action::Select {
        ids: ids(&["r1", "t1"]),
    });
    l.send(Action::MoveBy { dx: 10.0, dy: 0.0 });
    assert_eq!(get(&l.app, "t1").x, 10.0);
    l.send(Action::Undo {});
    assert_eq!((get(&l.app, "r1").x, get(&l.app, "t1").x), (0.0, 0.0));
    assert!(!l.app.state().history.can_undo);
}

#[test]
fn delete_removes_the_nodes_and_clears_them_from_the_selection() {
    let mut l = Loopback::live();
    l.send(Action::Select {
        ids: ids(&["r1", "t1"]),
    });
    l.send(Action::Delete { ids: ids(&["r1"]) });
    assert!(l.app.nodes().iter().all(|n| n.id != "r1"));
    assert_eq!(l.app.state().selection, ids(&["t1"]));
}

#[test]
fn reorder_changes_the_sibling_order() {
    let mut l = Loopback::live();
    let before = get(&l.app, "r1").pos;
    l.send(Action::Reorder {
        id: "r1".into(),
        before: None,
        after: Some("t1".into()),
    });
    assert_ne!(get(&l.app, "r1").pos, before);
    assert!(get(&l.app, "r1").pos > get(&l.app, "t1").pos);
}

#[test]
fn undo_and_redo_own_changes_drive_the_history_buttons() {
    let mut l = Loopback::live();
    assert!(!l.app.state().history.can_undo);
    l.send(Action::Move {
        id: "r1".into(),
        x: 9.0,
        y: 9.0,
    });
    let h = l.app.state().history;
    assert!((h.can_undo, h.can_redo, h.undo_steps) == (true, false, 1));
    let eff = l.send(Action::Undo {});
    assert_eq!(get(&l.app, "r1").x, 0.0);
    assert!(matches!(
        sent(&eff).as_slice(),
        [Command::UpdateNode { .. }]
    ));
    let h = l.app.state().history;
    assert!((h.can_undo, h.can_redo, h.redo_steps) == (false, true, 1));
    l.send(Action::Redo {});
    assert_eq!(get(&l.app, "r1").x, 9.0);
    assert!(
        l.events
            .iter()
            .any(|e| matches!(e, Event::HistoryChanged { .. }))
    );
}

#[test]
fn undo_with_nothing_to_undo_does_nothing() {
    let mut l = Loopback::live();
    assert!(l.app.dispatch(Action::Undo {}).is_empty());
    assert!(l.app.dispatch(Action::Redo {}).is_empty());
}

#[test]
fn a_refused_action_changes_nothing_and_says_why() {
    let mut l = Loopback::live();
    l.app.take_events();
    let eff = l.app.dispatch(Action::CreateNode {
        kind: CreateKind::Text,
        x: 0.0,
        y: 0.0,
        parent: Some("r1".into()), // a rectangle holds no children
        w: None,
        h: None,
    });
    assert!(eff.is_empty());
    assert_eq!(l.app.nodes().len(), 3);
    assert_eq!(
        l.app.take_events(),
        vec![Event::Refused {
            code: "bad_parent".into()
        }]
    );
    l.app.dispatch(Action::Move {
        id: "ghost".into(),
        x: 1.0,
        y: 1.0,
    });
    assert_eq!(
        l.app.take_events(),
        vec![Event::Refused {
            code: "not_found".into()
        }]
    );
}

#[test]
fn pending_ops_show_before_the_ack_and_clear_after_it() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    let eff = app.dispatch(Action::Move {
        id: "r1".into(),
        x: 3.0,
        y: 0.0,
    });
    let ops = ops_of(&eff);
    assert_eq!(get(&app, "r1").x, 3.0, "optimistic view");
    assert_eq!(app.state().pending, vec![ops[0].key.clone()]);
    assert_eq!(app.state().confirmed_seq, 0);
    app.take_events();
    app.dispatch(Action::Frame {
        now_ms: 1,
        frame: Frame::Event {
            seq: 1,
            actor: "user:me".into(),
            key: None,
            command: ops[0].command.clone(),
        },
    });
    assert!(app.state().pending.is_empty());
    assert_eq!(app.state().confirmed_seq, 1);
    assert_eq!(app.state().last_actor["r1"], "user:me");
}

#[test]
fn a_rejected_op_leaves_the_view_and_raises_an_event() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    let eff = app.dispatch(Action::Move {
        id: "r1".into(),
        x: 3.0,
        y: 0.0,
    });
    let ops = ops_of(&eff);
    app.take_events();
    app.dispatch(Action::Frame {
        now_ms: 1,
        frame: Frame::Rejected {
            key: ops[0].key.clone(),
            code: "conflict".into(),
            retryable: false,
        },
    });
    assert_eq!(get(&app, "r1").x, 0.0);
    assert!(app.state().pending.is_empty());
    assert!(app.take_events().contains(&Event::OpRejected {
        key: ops[0].key.clone(),
        code: "conflict".into(),
        node: None,
        retryable: false
    }));
}

#[test]
fn another_actors_change_is_applied_and_labelled() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(Action::Select { ids: ids(&["t1"]) });
    app.take_events();
    app.dispatch(Action::Frame {
        now_ms: 1,
        frame: Frame::Event {
            seq: 1,
            actor: "agent:claude".into(),
            key: None,
            command: Command::UpdateNode {
                id: "t1".into(),
                patch: meldframe_protocol::NodePatch {
                    text: Some("from agent".into()),
                    ..Default::default()
                },
            },
        },
    });
    assert_eq!(get(&app, "t1").text, "from agent");
    assert_eq!(app.state().last_actor["t1"], "agent:claude");
    assert_eq!(app.state().properties.clone().unwrap().text, "from agent");
    assert!(app.take_events().contains(&Event::DocumentChanged {
        actor: "agent:claude".into(),
        optimistic: false
    }));
}

#[test]
fn a_node_deleted_by_another_actor_leaves_the_selection() {
    let mut app = app();
    app.dispatch(Action::Select { ids: ids(&["r1"]) });
    app.dispatch(Action::Frame {
        now_ms: 1,
        frame: Frame::Event {
            seq: 1,
            actor: "agent:claude".into(),
            key: None,
            command: Command::RemoveNode { id: "r1".into() },
        },
    });
    assert!(app.state().selection.is_empty());
    assert!(!app.state().last_actor.contains_key("r1"));
}

#[test]
fn create_can_carry_a_size_and_stays_one_op() {
    let mut l = Loopback::live();
    let eff = l.send(Action::CreateNode {
        kind: CreateKind::Rectangle,
        x: 10.0,
        y: 20.0,
        parent: Some("root".into()),
        w: Some(120.0),
        h: Some(40.0),
    });
    let id = l.app.state().selection[0].clone();
    let n = get(&l.app, &id);
    assert_eq!((n.x, n.y, n.w, n.h), (10.0, 20.0, 120.0, 40.0));
    assert!(matches!(sent(&eff).as_slice(), [Command::AddNode { .. }]));
}

#[test]
fn a_size_that_is_not_valid_refuses_the_create_and_makes_nothing() {
    let mut l = Loopback::live();
    l.app.take_events();
    let before = l.app.nodes().len();
    for (w, h) in [(f64::NAN, 10.0), (10.0, f64::INFINITY), (-5.0, 10.0)] {
        let eff = l.app.dispatch(Action::CreateNode {
            kind: CreateKind::Rectangle,
            x: 0.0,
            y: 0.0,
            parent: Some("root".into()),
            w: Some(w),
            h: Some(h),
        });
        assert!(eff.is_empty(), "{w} x {h} must be refused");
    }
    assert_eq!(l.app.nodes().len(), before);
    assert!(
        l.app
            .take_events()
            .iter()
            .all(|e| matches!(e, Event::Refused { .. }))
    );
}

#[test]
fn resize_can_move_the_origin_in_the_same_step() {
    let mut l = Loopback::live();
    let before = get(&l.app, "r1");
    let eff = l.send(Action::Resize {
        id: "r1".into(),
        w: 30.0,
        h: 40.0,
        x: Some(5.0),
        y: Some(6.0),
    });
    let r = get(&l.app, "r1");
    assert_eq!((r.x, r.y, r.w, r.h), (5.0, 6.0, 30.0, 40.0));
    assert!(matches!(
        sent(&eff).as_slice(),
        [Command::UpdateNode { .. }]
    ));
    // One undo takes back the size and the origin together.
    l.send(Action::Undo {});
    let r = get(&l.app, "r1");
    assert_eq!(
        (r.x, r.y, r.w, r.h),
        (before.x, before.y, before.w, before.h)
    );
}

#[test]
fn resize_without_an_origin_keeps_the_position() {
    let mut l = Loopback::live();
    let before = get(&l.app, "r1");
    l.send(Action::Resize {
        id: "r1".into(),
        w: 30.0,
        h: 40.0,
        x: None,
        y: None,
    });
    let r = get(&l.app, "r1");
    assert_eq!((r.x, r.y, r.w, r.h), (before.x, before.y, 30.0, 40.0));
}

#[test]
fn resize_takes_one_origin_value_alone_and_refuses_one_that_is_not_finite() {
    let mut l = Loopback::live();
    let before = get(&l.app, "r1");
    l.send(Action::Resize {
        id: "r1".into(),
        w: 30.0,
        h: 40.0,
        x: Some(7.0),
        y: None,
    });
    let r = get(&l.app, "r1");
    assert_eq!((r.x, r.y, r.w, r.h), (7.0, before.y, 30.0, 40.0));
    l.app.take_events();
    let eff = l.app.dispatch(Action::Resize {
        id: "r1".into(),
        w: 1.0,
        h: 1.0,
        x: Some(f64::NAN),
        y: None,
    });
    assert!(eff.is_empty());
    assert!(matches!(
        l.app.take_events().as_slice(),
        [Event::Refused { .. }]
    ));
    assert_eq!(get(&l.app, "r1").w, 30.0);
}
