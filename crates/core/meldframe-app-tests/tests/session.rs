//! A scripted editing session: two editors and an agent on one file, through the real gate and room.

mod support;
use meldframe_app::{Action, CreateKind, Effect, Event, Property, Tool};
use meldframe_protocol::{Command, NodePatch};
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

fn text_patch(text: &str) -> NodePatch {
    NodePatch {
        text: Some(text.into()),
        ..Default::default()
    }
}

/// Alice and Bob open the file. Returns their indexes.
fn two_editors(w: &mut World) -> (usize, usize) {
    let a = w.join("user:alice", "a", 1);
    let b = w.join("user:bob", "b", 1);
    assert_eq!(w.app(a).state().connection, meldframe_app::Connection::Live);
    (a, b)
}

#[test]
fn a_person_builds_a_frame_a_rectangle_and_a_text_node_and_the_other_editor_sees_them() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.act(a, Action::SetTool { tool: Tool::Frame });
    w.act(a, create(CreateKind::Frame, 10.0, 10.0, "root"));
    let frame = selected(&w, a);
    w.act(a, create(CreateKind::Rectangle, 5.0, 5.0, &frame));
    let rect = selected(&w, a);
    w.act(a, create(CreateKind::Text, 8.0, 40.0, &frame));
    let text = selected(&w, a);
    w.pump_all();
    assert_eq!(w.app(a).state().tool, Tool::Select);
    for id in [&frame, &rect, &text] {
        assert!(
            w.app(b).nodes().iter().any(|n| n.id == *id),
            "bob sees {id}"
        );
        assert_eq!(w.app(b).state().last_actor[id], "user:alice");
    }
    assert!(w.clients[b].events.contains(&Event::DocumentChanged {
        actor: "user:alice".into(),
        optimistic: false
    }));
    w.assert_converged();
}

#[test]
fn move_resize_text_and_properties_reach_the_server_and_the_other_editor() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.act(a, create(CreateKind::Rectangle, 0.0, 0.0, "root"));
    let rect = selected(&w, a);
    w.act(
        a,
        Action::Move {
            id: rect.clone(),
            x: 30.0,
            y: 40.0,
        },
    );
    w.act(
        a,
        Action::Resize {
            id: rect.clone(),
            w: 200.0,
            h: 80.0,
        },
    );
    w.act(
        a,
        Action::SetProperty {
            id: rect.clone(),
            property: Property::Fill("#00ff00".into()),
        },
    );
    w.act(
        a,
        Action::SetProperty {
            id: rect.clone(),
            property: Property::Name("Card".into()),
        },
    );
    w.act(
        a,
        Action::EditText {
            id: "t1".into(),
            text: "Hello".into(),
        },
    );
    w.pump_all();
    let r = node_of(&w, b, &rect);
    assert_eq!((r.x, r.y, r.w, r.h), (30.0, 40.0, 200.0, 80.0));
    assert_eq!((r.fill.as_str(), r.name.as_str()), ("#00ff00", "Card"));
    assert_eq!(node_of(&w, b, "t1").text, "Hello");
    let p = w.app(a).state().properties.clone().unwrap();
    assert_eq!((p.w, p.name.as_str()), (200.0, "Card"));
    w.assert_converged();
}

#[test]
fn an_edit_shows_at_once_as_pending_and_clears_when_the_server_confirms_it() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.act(
        a,
        Action::EditText {
            id: "t1".into(),
            text: "draft".into(),
        },
    );
    assert_eq!(node_of(&w, a, "t1").text, "draft", "optimistic view");
    assert_eq!(w.app(a).state().pending.len(), 1);
    assert_eq!(
        w.app(a).base().iter().find(|n| n.id == "t1").unwrap().text,
        ""
    );
    assert_eq!(node_of(&w, b, "t1").text, "", "bob has not heard yet");
    w.pump_all();
    assert!(w.app(a).state().pending.is_empty());
    assert_eq!(w.app(a).state().confirmed_seq, w.room().1);
    assert_eq!(node_of(&w, b, "t1").text, "draft");
}

#[test]
fn undo_and_redo_touch_only_my_own_changes() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.act(a, create(CreateKind::Rectangle, 0.0, 0.0, "root"));
    let rect = selected(&w, a);
    w.act(
        a,
        Action::Move {
            id: rect.clone(),
            x: 50.0,
            y: 0.0,
        },
    );
    w.pump_all();
    w.act(
        b,
        Action::EditText {
            id: "t1".into(),
            text: "bob".into(),
        },
    );
    w.pump_all();
    assert!(!w.app(b).state().history.can_redo);
    w.act(a, Action::Undo {});
    w.pump_all();
    assert_eq!(node_of(&w, b, &rect).x, 0.0, "alice's move is undone");
    assert_eq!(node_of(&w, a, "t1").text, "bob", "bob's change stays");
    assert!(w.app(a).state().history.can_redo);
    w.act(a, Action::Redo {});
    w.pump_all();
    assert_eq!(node_of(&w, b, &rect).x, 50.0);
    assert_eq!(node_of(&w, a, "t1").text, "bob");
    w.assert_converged();
}

#[test]
fn an_agents_changes_appear_and_are_labelled_with_the_actor() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.act(
        a,
        Action::Select {
            ids: vec!["t1".into()],
        },
    );
    w.agent(vec![Command::UpdateNode {
        id: "t1".into(),
        patch: text_patch("from the agent"),
    }]);
    w.pump_all();
    for i in [a, b] {
        assert_eq!(node_of(&w, i, "t1").text, "from the agent");
        assert_eq!(w.app(i).state().last_actor["t1"], "agent:claude");
    }
    assert_eq!(
        w.app(a).state().properties.clone().unwrap().text,
        "from the agent"
    );
    assert!(w.clients[a].events.contains(&Event::DocumentChanged {
        actor: "agent:claude".into(),
        optimistic: false
    }));
    assert!(
        !w.app(a).state().history.can_undo,
        "the agent's change is not in my history"
    );
    w.assert_converged();
}

#[test]
fn a_node_the_agent_deletes_leaves_the_selection_and_the_layers() {
    let mut w = World::new(64);
    let (a, _) = two_editors(&mut w);
    w.act(
        a,
        Action::Select {
            ids: vec!["t1".into()],
        },
    );
    w.agent(vec![Command::RemoveNode { id: "t1".into() }]);
    w.pump_all();
    assert!(w.app(a).state().selection.is_empty());
    assert!(!w.app(a).state().last_actor.contains_key("t1"));
    w.assert_converged();
}

#[test]
fn a_dropped_connection_goes_offline_then_reconnects_and_catches_up() {
    let mut w = World::new(64);
    let (a, b) = two_editors(&mut w);
    w.act(
        a,
        Action::EditText {
            id: "t1".into(),
            text: "one".into(),
        },
    );
    w.pump_all();
    let before = w.app(a).state().confirmed_seq;
    w.drop_link(a);
    assert_eq!(
        w.app(a).state().connection,
        meldframe_app::Connection::Offline
    );
    assert!(
        w.clients[a]
            .effects
            .iter()
            .any(|e| matches!(e, Effect::Reconnect { after_ms: 1_000 }))
    );
    // Meanwhile: an agent and Bob write, and Alice edits offline.
    w.agent(vec![Command::UpdateNode {
        id: "root".into(),
        patch: NodePatch {
            name: Some("Page".into()),
            ..Default::default()
        },
    }]);
    w.act(
        b,
        Action::SetProperty {
            id: "t1".into(),
            property: Property::Fill("#ff00ff".into()),
        },
    );
    w.pump(b);
    w.act(a, create(CreateKind::Rectangle, 1.0, 2.0, "root"));
    let rect = selected(&w, a);
    assert_eq!(w.app(a).state().pending.len(), 1, "held, not sent");
    assert!(node_of(&w, a, &rect).x == 1.0, "shown at once");
    // The backoff is over.
    w.now += 1_000;
    w.act(a, Action::Reconnect { now_ms: w.now });
    use meldframe_app::Connection::*;
    assert_eq!(
        w.clients[a].states[1..],
        [Live, Offline, Reconnecting { attempt: 1 }, Live]
    );
    assert!(
        w.clients[a]
            .effects
            .iter()
            .any(|e| matches!(e, Effect::Subscribe { after_seq } if *after_seq == before))
    );
    w.pump_all();
    assert_eq!(node_of(&w, a, "root").name, "Page", "missed frames applied");
    assert_eq!(node_of(&w, a, "t1").fill, "#ff00ff");
    assert_eq!(w.app(a).state().last_actor["root"], "agent:claude");
    assert!(
        w.room().0.iter().any(|n| n.id == rect),
        "the offline edit reached the server"
    );
    w.assert_converged();
}

#[test]
fn a_resync_marker_makes_the_app_subscribe_again_from_the_marker() {
    let mut w = World::new(3);
    let (a, _b) = two_editors(&mut w);
    let start = w.app(a).state().confirmed_seq;
    for n in 1..=6 {
        w.agent(vec![Command::UpdateNode {
            id: "t1".into(),
            patch: text_patch(&format!("v{n}")),
        }]);
    }
    w.pump_all();
    let marker = start + 3;
    assert!(
        w.clients[a]
            .effects
            .iter()
            .any(|e| matches!(e, Effect::Subscribe { after_seq } if *after_seq == marker)),
        "{:?}",
        w.clients[a].effects
    );
    assert_eq!(node_of(&w, a, "t1").text, "v6");
    assert_eq!(w.app(a).state().connection, meldframe_app::Connection::Live);
    w.assert_converged();
}
