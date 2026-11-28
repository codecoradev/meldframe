//! A double click and the keys of a gesture, through the app with no front end (E8.1.4, part 2): a double click on a text begins its
//! edit and one on a rectangle does nothing, a press while the edit is open is the field's, and Escape and the arrow keys know
//! about the gesture that goes on (`AppState::pointer.gesture`).

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::pointer::*;
use layout_fixture::*;
use meldframe_app::{
    Action, App, Event, KeyFocus, KeyInput, Modifiers, PointerGesture, PointerInput, PointerPhase,
    TextEditOp,
};
use meldframe_protocol::Node;
use serde_json::json;

fn nodes_with_text() -> Vec<Node> {
    let mut v = nodes();
    v.push(
        serde_json::from_value(
            json!({"id": "t", "type": "text", "pos": "a3", "x": 1200, "y": 20, "w": 80, "h": 20, "text": "hi"}),
        )
        .expect("a text node"),
    );
    v
}

fn at(phase: PointerPhase, x: f64, y: f64, ms: u64) -> PointerInput {
    PointerInput {
        at_ms: Some(ms),
        ..input(phase, x, y)
    }
}

fn click_at(a: &mut App, x: f64, y: f64, ms: u64) {
    send(a, at(PointerPhase::Down, x, y, ms));
    send(a, at(PointerPhase::Up, x, y, ms));
}

fn key(a: &mut App, key: &str) -> Vec<Event> {
    a.take_events();
    a.dispatch(Action::Key {
        input: KeyInput {
            key: key.into(),
            code: String::new(),
            modifiers: Modifiers::default(),
            repeat: false,
            composing: false,
            focus: KeyFocus::Page,
        },
    });
    a.take_events()
}

#[test]
fn a_double_click_on_a_text_selects_it_and_begins_its_edit() {
    let mut a = app_of(nodes_with_text(), true);
    click_at(&mut a, 1210.0, 30.0, 1000);
    assert!(a.state().text_edit.is_none());
    assert_eq!(selection(&a), ["t"]);
    click_at(&mut a, 1210.0, 30.0, 1250);
    let edit = a.state().text_edit.clone().expect("the edit began");
    assert_eq!((edit.id.as_str(), edit.text.as_str()), ("t", "hi"));
    assert_eq!(selection(&a), ["t"]);
    assert_eq!(
        steps(&a),
        0,
        "an edit is not a change until it is committed"
    );
}

#[test]
fn a_double_click_on_a_rectangle_does_nothing_and_the_press_that_follows_an_open_edit_is_the_fields()
 {
    let mut a = app_of(nodes_with_text(), true);
    click_at(&mut a, 435.0, 35.0, 1000);
    click_at(&mut a, 435.0, 35.0, 1200);
    assert!(a.state().text_edit.is_none());
    assert_eq!(selection(&a), ["fg"]);
    // With an edit open the press is not the pointer's: the field commits first, and then the press goes on.
    a.dispatch(Action::TextEdit {
        op: TextEditOp::Begin { id: "t".into() },
    });
    click_at(&mut a, 435.0, 35.0, 2000);
    assert!(a.state().text_edit.is_some());
    assert_eq!(selection(&a), ["t"]);
    a.dispatch(Action::TextEdit {
        op: TextEditOp::Commit {
            id: "t".into(),
            text: "hello".into(),
        },
    });
    assert!(a.state().text_edit.is_none());
    click_at(&mut a, 435.0, 35.0, 2100);
    assert_eq!(selection(&a), ["fg"], "the press after the commit selects");
}

#[test]
fn escape_abandons_the_gesture_that_goes_on_and_is_the_editors() {
    let mut a = app(true);
    press(&mut a, 435.0, 35.0);
    moved(&mut a, 480.0, 90.0);
    assert!(matches!(
        a.state().pointer.gesture,
        PointerGesture::Move { .. }
    ));
    let events = key(&mut a, "Escape");
    assert!(events.contains(&Event::KeyHandled {}));
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    assert!(release(&mut a, 480.0, 90.0).is_empty());
    assert_eq!(steps(&a), 0);
    // With nothing going on Escape is not taken here.
    assert!(!key(&mut a, "Escape").contains(&Event::KeyHandled {}));
}

#[test]
fn an_arrow_key_is_not_a_command_while_a_gesture_goes_on() {
    let mut a = app(true);
    a.dispatch(Action::Select {
        ids: vec!["b".into()],
    });
    press(&mut a, 30.0, 50.0); // `b`, a flow child: it is selected already
    moved(&mut a, 30.0, 40.0);
    assert!(matches!(
        a.state().pointer.gesture,
        PointerGesture::Move { .. }
    ));
    let events = key(&mut a, "ArrowUp");
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(kids(&a, Some("f")), ["a", "b", "c", "abs"]);
    send(&mut a, input(PointerPhase::Cancel, 30.0, 40.0));
    let events = key(&mut a, "ArrowUp");
    assert!(events.contains(&Event::KeyHandled {}), "{events:?}");
    assert_eq!(kids(&a, Some("f")), ["b", "a", "c", "abs"]);
}

// --- Escape before the drag has begun ------------------------------------------------------------------------------

fn middle(phase: PointerPhase, x: f64, y: f64) -> PointerInput {
    PointerInput {
        button: meldframe_app::PointerButton::Middle,
        ..input(phase, x, y)
    }
}

#[test]
fn escape_after_the_press_of_a_drawing_tool_and_before_the_drag_makes_nothing() {
    let mut a = app(true);
    a.dispatch(Action::SetTool {
        tool: meldframe_app::Tool::Rectangle,
    });
    press(&mut a, 900.0, 500.0);
    let events = key(&mut a, "Escape");
    assert!(events.contains(&Event::KeyHandled {}), "{events:?}");
    assert!(release(&mut a, 900.0, 500.0).is_empty());
    assert_eq!(a.nodes().len(), 10, "no node");
    assert_eq!(steps(&a), 0);
}

#[test]
fn escape_after_a_press_on_a_node_or_on_nothing_leaves_the_selection_to_the_press() {
    let mut a = app(true);
    click(&mut a, 30.0, 20.0);
    press(&mut a, 900.0, 500.0); // on nothing: the release would clear the selection
    assert!(key(&mut a, "Escape").contains(&Event::KeyHandled {}));
    release(&mut a, 900.0, 500.0);
    assert_eq!(selection(&a), ["a"], "the abandoned press clears nothing");
    press(&mut a, 435.0, 35.0); // on a node: selected at once, and not dragged
    assert!(key(&mut a, "Escape").contains(&Event::KeyHandled {}));
    moved(&mut a, 480.0, 90.0);
    assert!(
        release(&mut a, 480.0, 90.0).is_empty(),
        "the node is not dragged after it"
    );
    assert_eq!(steps(&a), 0);
}

#[test]
fn escape_during_a_pan_ends_it_and_after_a_resize_press_makes_no_resize() {
    let mut a = app(true);
    send(&mut a, middle(PointerPhase::Down, 100.0, 100.0));
    moved(&mut a, 120.0, 100.0);
    assert_eq!(a.state().camera.x, 20.0);
    assert!(key(&mut a, "Escape").contains(&Event::KeyHandled {}));
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    moved(&mut a, 160.0, 100.0);
    assert_eq!(a.state().camera.x, 20.0, "the camera stays");
    click(&mut a, 435.0, 35.0);
    press(&mut a, 450.0, 50.0); // the south east handle of `fg`, not yet dragged
    assert!(key(&mut a, "Escape").contains(&Event::KeyHandled {}));
    moved(&mut a, 480.0, 90.0);
    assert!(release(&mut a, 480.0, 90.0).is_empty());
    assert_eq!(steps(&a), 0);
}

#[test]
fn escape_with_nothing_pressed_is_not_the_pointers() {
    let mut a = app(true);
    moved(&mut a, 30.0, 20.0);
    assert!(!key(&mut a, "Escape").contains(&Event::KeyHandled {}));
}

#[test]
fn a_double_click_needs_the_same_node_under_both_clicks() {
    let mut nodes = nodes_with_text();
    nodes.push(
        serde_json::from_value(
            json!({"id": "u", "type": "text", "pos": "a4", "x": 1280, "y": 20, "w": 80, "h": 20, "text": "yo"}),
        )
        .expect("a text node"),
    );
    let mut a = app_of(nodes, true);
    click_at(&mut a, 1279.0, 30.0, 1000); // the last pixel of `t`
    click_at(&mut a, 1281.0, 30.0, 1100); // the first of `u`, 2 px and 100 ms on
    assert!(
        a.state().text_edit.is_none(),
        "two nodes, not one clicked twice"
    );
    assert_eq!(selection(&a), ["u"]);
    click_at(&mut a, 1281.0, 30.0, 1200);
    assert_eq!(
        a.state().text_edit.clone().map(|e| e.id),
        Some("u".to_string())
    );
}
