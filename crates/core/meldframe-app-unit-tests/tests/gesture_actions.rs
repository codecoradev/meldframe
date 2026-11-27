//! `Action::Gesture` (T8, decision 43): its wire shape, and what it does while the app is halted and while a text is being
//! edited. The geometry is tested in `meldframe-app-layout-tests`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Connection, CreateKind, Event, GestureOp, Handle, TextEditOp};
use meldframe_protocol::{Command, NodePatch};
use serde_json::json;

fn draw_json() -> serde_json::Value {
    json!({"type": "gesture", "op": {"type": "draw", "kind": "ellipse",
        "from": {"x": 1.5, "y": 2.0}, "to": {"x": 10.0, "y": 20.0}}})
}

#[test]
fn a_gesture_reads_from_json_with_the_keys_left_out() {
    let a: Action = serde_json::from_value(draw_json()).unwrap();
    let Action::Gesture {
        op: GestureOp::Draw {
            kind, from, mods, ..
        },
    } = a
    else {
        panic!("a draw")
    };
    assert_eq!(kind, CreateKind::Ellipse);
    assert_eq!(from.x, 1.5);
    assert!(!(mods.shift || mods.alt || mods.ctrl || mods.meta));
    let a: Action = serde_json::from_value(json!({"type": "gesture", "op": {
        "type": "draw", "kind": "frame", "from": {"x": 0, "y": 0}, "to": {"x": 1, "y": 1},
        "mods": {"shift": true}}}))
    .unwrap();
    assert!(
        matches!(a, Action::Gesture { op: GestureOp::Draw { mods, .. } } if mods.shift && !mods.alt)
    );
}

#[test]
fn a_handle_reads_by_its_compass_name_and_every_kind_round_trips() {
    for (name, want) in [
        ("nw", Handle::Nw),
        ("n", Handle::N),
        ("ne", Handle::Ne),
        ("e", Handle::E),
        ("se", Handle::Se),
        ("s", Handle::S),
        ("sw", Handle::Sw),
        ("w", Handle::W),
    ] {
        let a: Action = serde_json::from_value(json!({"type": "gesture", "op": {
            "type": "resizeHandle", "id": "r1", "handle": name, "by": {"x": 1, "y": 2}}}))
        .unwrap();
        assert!(
            matches!(a, Action::Gesture { op: GestureOp::ResizeHandle { handle, .. } } if handle == want)
        );
    }
    assert!(
        serde_json::from_value::<Action>(json!({"type": "gesture", "op": {
            "type": "resizeHandle", "id": "r1", "handle": "NW", "by": {"x": 1, "y": 2}}}))
        .is_err()
    );
    for kind in ["frame", "rectangle", "ellipse", "text"] {
        let mut j = draw_json();
        j["op"]["kind"] = json!(kind);
        let a: Action = serde_json::from_value(j.clone()).unwrap();
        let back = serde_json::to_value(&a).unwrap();
        assert_eq!(back["op"]["kind"], j["op"]["kind"]);
        assert_eq!(back["op"]["from"], j["op"]["from"]);
        assert_eq!(
            back["op"]["mods"]["shift"],
            json!(false),
            "the keys are written out"
        );
    }
}

#[test]
fn unknown_fields_and_kinds_are_refused() {
    let mut j = draw_json();
    j["extra"] = json!(1);
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["extra"] = json!(1);
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["from"]["z"] = json!(1);
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["mods"] = json!({"super": true});
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["kind"] = json!("star");
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["type"] = json!("lasso");
    assert!(serde_json::from_value::<Action>(j).is_err());
}

fn draw(from: (f64, f64), to: (f64, f64)) -> Action {
    serde_json::from_value(
        json!({"type": "gesture", "op": {"type": "draw", "kind": "rectangle",
        "from": {"x": from.0, "y": from.1}, "to": {"x": to.0, "y": to.1}}}),
    )
    .unwrap()
}

fn drag(id: &str) -> Action {
    serde_json::from_value(
        json!({"type": "gesture", "op": {"type": "resizeHandle", "id": id,
        "handle": "se", "by": {"x": 5, "y": 5}}}),
    )
    .unwrap()
}

#[test]
fn a_halted_app_does_nothing_for_a_gesture() {
    let mut l = Loopback::live();
    // A change of a node the app does not have diverges it.
    l.remote(
        "agent:a",
        "k",
        1,
        Command::UpdateNode {
            id: "ghost".into(),
            patch: NodePatch::default(),
        },
    );
    assert_eq!(l.app.state().connection, Connection::Halted);
    l.events.clear();
    let before = serde_json::to_value(l.app.nodes()).unwrap();
    for a in [draw((2000.0, 0.0), (2040.0, 30.0)), drag("r1")] {
        let fx = l.send(a);
        assert!(fx.is_empty());
    }
    assert_eq!(serde_json::to_value(l.app.nodes()).unwrap(), before);
    assert!(l.events.is_empty(), "no event: {:?}", l.events);
}

#[test]
fn a_draw_while_a_text_is_edited_leaves_the_edit_open_as_any_new_node_does() {
    // Nothing closes the edit but a commit, a cancel or the loss of its node: the field reports its text when it loses
    // focus, which a press on the canvas makes it do. The draw selects the new node and the edit stays on its text.
    let mut l = Loopback::live();
    l.send(Action::TextEdit {
        op: TextEditOp::Begin { id: "t1".into() },
    });
    l.events.clear();
    l.send(draw((2000.0, 0.0), (2040.0, 30.0)));
    l.events.extend(l.app.take_events());
    assert_eq!(
        l.app.state().text_edit.as_ref().map(|e| e.id.as_str()),
        Some("t1")
    );
    assert_eq!(l.app.nodes().len(), doc().len() + 1);
    assert_ne!(l.app.state().selection, vec!["t1".to_string()]);
    assert!(!l.events.iter().any(|e| matches!(e, Event::Refused { .. })));
}
