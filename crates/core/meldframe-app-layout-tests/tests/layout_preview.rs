//! The preview of a drop on a laid-out canvas: where in a flow the node would go, as data a front end draws a line from.
//! It answers from the same rule as the drop (`drop_commands`), so what is shown is what a release does. With the flag
//! off, or for a drop that does not go into a flow, there is none.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::DropSlot;

fn preview(a: &meldframe_app::App, id: &str, at: (f64, f64)) -> Option<DropSlot> {
    a.drop_preview(id, at, (0.0, 0.0))
}

fn line(s: &DropSlot) -> (f64, f64, f64, f64) {
    (s.line.x, s.line.y, s.line.w, s.line.h)
}

#[test]
fn between_two_children_of_a_column_the_line_is_across_the_gap() {
    let a = app(true);
    // `a` is dragged; `b` (35 to 65) and `c` (70 to 110) remain, the pointer is past the middle of `b` only.
    let s = preview(&a, "a", (30.0, 60.0)).expect("a slot");
    assert_eq!((s.frame.as_str(), s.index), ("f", 1));
    assert_eq!(
        line(&s),
        (10.0, 67.5, 50.0, 0.0),
        "the middle of the gap, as wide as the neighbour"
    );
}

#[test]
fn before_the_first_and_after_the_last_the_line_is_on_the_edge_of_the_neighbour() {
    let a = app(true);
    let s = preview(&a, "c", (30.0, 12.0)).expect("a slot");
    assert_eq!((s.index, line(&s)), (0, (10.0, 10.0, 50.0, 0.0)));
    let s = preview(&a, "a", (30.0, 280.0)).expect("a slot");
    assert_eq!((s.index, line(&s)), (2, (10.0, 110.0, 50.0, 0.0)));
}

#[test]
fn in_a_row_the_line_is_vertical() {
    let a = app(true);
    let s = preview(&a, "r2", (710.0, 50.0)).expect("a slot");
    assert_eq!((s.frame.as_str(), s.index), ("r", 0));
    assert_eq!(line(&s), (716.0, 16.0, 0.0, 40.0));
}

#[test]
fn over_another_auto_layout_frame_the_slot_is_in_that_frame() {
    let a = app(true);
    let s = preview(&a, "a", (720.0, 50.0)).expect("a slot");
    assert_eq!((s.frame.as_str(), s.index), ("r", 0));
}

#[test]
fn an_empty_auto_layout_frame_has_its_only_slot_at_its_start() {
    let a = app_of(cards(), true);
    let s = preview(&a, "k", (520.0, 420.0)).expect("a slot");
    assert_eq!((s.frame.as_str(), s.index), ("empty", 0));
    // `empty` is a column at (500, 400), 200 by 100, with the default padding of 16: the line is inside it.
    assert_eq!(line(&s), (516.0, 416.0, 168.0, 0.0));
}

#[test]
fn there_is_none_when_the_drop_does_not_go_into_a_flow() {
    let a = app(true);
    // Out of the flow to the page, a node of a free frame inside its own frame, an absolute node, an unknown node.
    assert_eq!(preview(&a, "b", (1500.0, 900.0)), None);
    assert_eq!(preview(&a, "fg", (420.0, 40.0)), None);
    assert_eq!(preview(&a, "abs", (30.0, 60.0)), None);
    assert_eq!(preview(&a, "nope", (30.0, 60.0)), None);
    assert_eq!(a.drop_preview("a", (f64::NAN, 1.0), (0.0, 0.0)), None);
}

#[test]
fn with_the_flag_off_there_is_none() {
    let a = app(false);
    assert_eq!(preview(&a, "a", (30.0, 60.0)), None);
}

#[test]
fn the_slot_is_where_the_drop_puts_the_node() {
    for y in [0.0, 12.0, 25.0, 40.0, 60.0, 80.0, 100.0, 120.0, 280.0] {
        let mut a = app(true);
        let slot = preview(&a, "a", (30.0, y)).expect("a slot");
        drop_node(&mut a, "a", (30.0, y), (0.0, 0.0));
        let flow: Vec<String> = kids(&a, Some("f"))
            .into_iter()
            .filter(|k| k != "abs")
            .collect();
        assert_eq!(
            flow.iter().position(|k| k == "a"),
            Some(slot.index),
            "at y {y}"
        );
    }
}

#[test]
fn a_preview_changes_nothing() {
    // `drop_preview` borrows the app shared, so the type already says it; this checks the state, the file, the history
    // and the events byte for byte over a preview that gives a slot, one that gives none and one that is refused.
    let mut a = app(true);
    let before = (
        serde_json::to_string(a.state()).unwrap(),
        serde_json::to_string(&a.nodes()).unwrap(),
        steps(&a),
    );
    assert!(preview(&a, "a", (30.0, 60.0)).is_some());
    assert!(preview(&a, "a", (1500.0, 900.0)).is_none());
    assert!(preview(&a, "nope", (30.0, 60.0)).is_none());
    assert!(a.drop_preview("a", (f64::NAN, 1.0), (0.0, 0.0)).is_none());
    assert_eq!(
        before,
        (
            serde_json::to_string(a.state()).unwrap(),
            serde_json::to_string(&a.nodes()).unwrap(),
            steps(&a)
        )
    );
    assert!(a.take_events().is_empty(), "a preview raises no event");
    assert!(a.state().pending.is_empty(), "and sends no op");
}
