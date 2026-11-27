//! `Action::Gesture` (T8, decision 43): what it does while the app is halted and while a text is being edited. The geometry
//! is tested in `meldframe-app-layout-tests`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Connection, CreateKind, Event, Handle, Modifiers, Point, TextEditOp};
use meldframe_protocol::{Command, NodePatch};
use serde_json::json;

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
    // A preview is what a release would make, and a release does nothing now: so there is none.
    let (from, to) = (Point { x: 2000.0, y: 0.0 }, Point { x: 2040.0, y: 30.0 });
    let by = Point { x: 5.0, y: 5.0 };
    assert!(
        l.app
            .draw_preview(CreateKind::Rectangle, from, to, Modifiers::default())
            .is_none()
    );
    assert!(
        l.app
            .resize_preview("r1", Handle::Se, by, Modifiers::default())
            .is_none()
    );
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
