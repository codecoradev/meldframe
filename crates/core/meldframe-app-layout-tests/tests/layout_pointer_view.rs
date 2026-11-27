//! The pointer on a laid-out canvas with no front end, the view side (E8.1.4, part 2): a pan and a zoom to the pointer, hover and
//! cursor, and what a cancel, a leave, a halted app, a text edit and a point that is not a number do, read from `AppState`.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::pointer::*;
use layout_fixture::*;

use meldframe_app::{
    Action, App, CameraOp, Connection, Event, Modifiers, Point, PointerButton, PointerCursor,
    PointerGesture, PointerInput, PointerPhase, Tool, Wheel, WheelUnit,
};

// --- pan and zoom --------------------------------------------------------------------------------------------------

#[test]
fn the_middle_button_and_space_pan_the_camera_and_the_wheel_zooms_around_the_pointer() {
    let mut a = app(true);
    let down = PointerInput {
        button: PointerButton::Middle,
        ..input(PointerPhase::Down, 100.0, 100.0)
    };
    send(&mut a, down);
    assert_eq!(a.state().pointer.cursor, PointerCursor::Grabbing);
    moved(&mut a, 130.0, 90.0);
    moved(&mut a, 150.0, 120.0);
    assert_eq!((a.state().camera.x, a.state().camera.y), (50.0, 20.0));
    release(&mut a, 150.0, 120.0);
    assert_eq!(a.state().pointer.cursor, PointerCursor::Default);
    assert_eq!(steps(&a), 0);
    assert!(selection(&a).is_empty(), "a pan selects nothing");
    // Space and the primary button pan too, over a node.
    let space = |phase, x, y| PointerInput {
        space: true,
        ..input(phase, x, y)
    };
    send(&mut a, space(PointerPhase::Move, 80.0, 80.0));
    assert_eq!(a.state().pointer.cursor, PointerCursor::Grab);
    send(&mut a, space(PointerPhase::Down, 80.0, 80.0));
    send(&mut a, space(PointerPhase::Move, 90.0, 80.0));
    assert_eq!((a.state().camera.x, a.state().camera.y), (60.0, 20.0));
    send(&mut a, space(PointerPhase::Up, 90.0, 80.0));
    // The wheel with Ctrl zooms to the pointer: the world point under it stays under it.
    let under = |a: &App| {
        let c = a.state().camera;
        ((200.0 - c.x) / c.zoom, (150.0 - c.y) / c.zoom)
    };
    let before = under(&a);
    camera(
        &mut a,
        CameraOp::Wheel {
            wheel: Wheel {
                dx: 0.0,
                dy: -20.0,
                unit: WheelUnit::Pixel,
                mods: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            },
            at: Point { x: 200.0, y: 150.0 },
        },
    );
    assert!(a.state().camera.zoom > 1.0);
    let after = under(&a);
    assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
}

#[test]
fn a_finger_that_drags_on_empty_canvas_pans() {
    let mut a = app(true);
    let touch = |phase, x, y| PointerInput {
        touch: true,
        ..input(phase, x, y)
    };
    send(&mut a, touch(PointerPhase::Down, 900.0, 500.0));
    send(&mut a, touch(PointerPhase::Move, 910.0, 520.0));
    assert_eq!((a.state().camera.x, a.state().camera.y), (10.0, 20.0));
    send(&mut a, touch(PointerPhase::Up, 910.0, 520.0));
    assert_eq!(steps(&a), 0);
    // A mouse has nothing to drag there.
    press(&mut a, 900.0, 500.0);
    moved(&mut a, 930.0, 520.0);
    assert_eq!((a.state().camera.x, a.state().camera.y), (10.0, 20.0));
}

// --- hover and cursor ----------------------------------------------------------------------------------------------

#[test]
fn hover_is_the_node_under_the_pointer_and_the_event_is_raised_for_a_change_only() {
    let mut a = app(true);
    a.take_events();
    moved(&mut a, 30.0, 20.0);
    assert_eq!(a.state().pointer.hover.as_deref(), Some("a"));
    assert_eq!(a.take_events(), vec![Event::PointerChanged {}]);
    moved(&mut a, 32.0, 21.0);
    assert!(
        a.take_events().is_empty(),
        "the same node: nothing to draw again"
    );
    moved(&mut a, 900.0, 500.0);
    assert_eq!(a.state().pointer.hover, None);
    moved(&mut a, 30.0, 20.0);
    send(&mut a, input(PointerPhase::Leave, 30.0, 20.0));
    assert_eq!(a.state().pointer.hover, None);
}

#[test]
fn a_tool_that_draws_shows_the_crosshair_and_no_hover() {
    let mut a = app(true);
    a.dispatch(Action::SetTool {
        tool: Tool::Rectangle,
    });
    moved(&mut a, 30.0, 20.0);
    assert_eq!(a.state().pointer.hover, None);
    assert_eq!(a.state().pointer.cursor, PointerCursor::Crosshair);
    a.dispatch(Action::SetTool { tool: Tool::Select });
    moved(&mut a, 30.0, 20.0);
    assert_eq!(a.state().pointer.cursor, PointerCursor::Default);
}

// --- cancel, leave, halted, text edit, bad numbers -----------------------------------------------------------------

#[test]
fn a_cancel_and_a_leave_end_the_drag_and_leave_the_document_alone() {
    for phase in [PointerPhase::Cancel, PointerPhase::Leave] {
        let mut a = app(true);
        let before = serde_json::to_value(a.nodes()).unwrap();
        press(&mut a, 435.0, 35.0);
        moved(&mut a, 480.0, 90.0);
        assert!(matches!(
            a.state().pointer.gesture,
            PointerGesture::Move { .. }
        ));
        let fx = send(&mut a, input(phase, 480.0, 90.0));
        assert!(fx.is_empty());
        assert_eq!(a.state().pointer.gesture, PointerGesture::Idle, "{phase:?}");
        assert_eq!(a.state().pointer.hover, None);
        assert!(
            release(&mut a, 480.0, 90.0).is_empty(),
            "the release after it is nothing"
        );
        assert_eq!(serde_json::to_value(a.nodes()).unwrap(), before);
        assert_eq!(steps(&a), 0);
    }
}

#[test]
fn a_press_while_a_drag_goes_on_means_the_release_was_lost_and_starts_afresh() {
    let mut a = app(true);
    press(&mut a, 435.0, 35.0);
    moved(&mut a, 480.0, 90.0);
    press(&mut a, 30.0, 20.0);
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    assert_eq!(selection(&a), ["a"]);
    assert_eq!(steps(&a), 0, "the first drag was dropped, not applied");
}

#[test]
fn a_halted_app_still_selects_and_pans_and_writes_nothing() {
    let mut a = app(true);
    halt(&mut a);
    assert_eq!(a.state().connection, Connection::Halted);
    a.take_events();
    let before = serde_json::to_value(a.nodes()).unwrap();
    press(&mut a, 435.0, 35.0);
    assert_eq!(selection(&a), ["fg"], "a selection is view state");
    moved(&mut a, 480.0, 90.0);
    assert_eq!(
        a.state().pointer.gesture,
        PointerGesture::Idle,
        "no drag begins"
    );
    assert!(release(&mut a, 480.0, 90.0).is_empty());
    assert_eq!(serde_json::to_value(a.nodes()).unwrap(), before);
    let middle = PointerInput {
        button: PointerButton::Middle,
        ..input(PointerPhase::Down, 10.0, 10.0)
    };
    send(&mut a, middle);
    moved(&mut a, 20.0, 10.0);
    assert_eq!(a.state().camera.x, 10.0);
}

#[test]
fn a_press_that_finds_a_text_edit_open_is_dropped_because_the_field_commits_before_it_is_sent() {
    let nodes = vec![
        serde_json::from_value(serde_json::json!(
            {"id": "t", "type": "text", "pos": "a0", "w": 80, "h": 20, "text": "hi"}
        ))
        .unwrap(),
    ];
    let mut a = app_of(nodes, true);
    a.dispatch(Action::TextEdit {
        op: meldframe_app::TextEditOp::Begin { id: "t".into() },
    });
    assert!(a.state().text_edit.is_some());
    a.dispatch(Action::ClearSelection {});
    press(&mut a, 10.0, 10.0);
    release(&mut a, 10.0, 10.0);
    assert!(selection(&a).is_empty());
    assert!(
        a.state().text_edit.is_some(),
        "the field commits when it loses the focus"
    );
}

#[test]
fn a_point_that_is_not_a_number_changes_nothing() {
    let mut a = app(true);
    press(&mut a, 435.0, 35.0);
    let before = a.state().clone();
    a.take_events();
    for (x, y) in [
        (f64::NAN, 5.0),
        (5.0, f64::INFINITY),
        (f64::NEG_INFINITY, f64::NAN),
    ] {
        for phase in [PointerPhase::Down, PointerPhase::Move, PointerPhase::Up] {
            assert!(send(&mut a, input(phase, x, y)).is_empty());
        }
    }
    assert_eq!(*a.state(), before);
    assert!(a.take_events().is_empty());
    // A finite point so far out that the world point is not a number is refused the same way.
    send(&mut a, input(PointerPhase::Move, 1e308, 1e308));
    camera(
        &mut a,
        CameraOp::ZoomAt {
            factor: 1e-300,
            at: Point { x: 0.0, y: 0.0 },
        },
    );
    let at = a.state().clone();
    send(&mut a, input(PointerPhase::Move, 1e308, 1e308));
    assert_eq!(*a.state(), at);
    assert_eq!(steps(&a), 0);
}

// --- what the pointer shows follows the app ------------------------------------------------------------------------

#[test]
fn a_change_of_the_tool_shows_in_the_cursor_without_a_move() {
    let mut a = app(true);
    moved(&mut a, 30.0, 20.0);
    assert_eq!(a.state().pointer.cursor, PointerCursor::Default);
    assert_eq!(a.state().pointer.hover.as_deref(), Some("a"));
    a.take_events();
    a.dispatch(Action::SetTool {
        tool: Tool::Rectangle,
    });
    assert_eq!(a.state().pointer.cursor, PointerCursor::Crosshair);
    assert_eq!(a.state().pointer.hover, None);
    assert!(a.take_events().contains(&Event::PointerChanged {}));
    // The key that picks the tool is the same change.
    a.dispatch(Action::SetTool { tool: Tool::Select });
    assert_eq!(a.state().pointer.hover.as_deref(), Some("a"));
    // Once the pointer left there is nothing to show.
    send(&mut a, input(PointerPhase::Leave, 30.0, 20.0));
    a.dispatch(Action::SetTool { tool: Tool::Frame });
    assert_eq!(a.state().pointer.hover, None);
    assert_eq!(a.state().pointer.cursor, PointerCursor::Default);
}

#[test]
fn a_node_that_is_deleted_leaves_the_hover() {
    let mut a = app(true);
    moved(&mut a, 435.0, 35.0);
    assert_eq!(a.state().pointer.hover.as_deref(), Some("fg"));
    a.take_events();
    a.dispatch(Action::Delete {
        ids: vec!["fg".into()],
    });
    assert_eq!(a.state().pointer.hover, None);
    assert!(a.take_events().contains(&Event::PointerChanged {}));
    // Another node's removal leaves it be.
    moved(&mut a, 30.0, 20.0);
    a.dispatch(Action::Delete {
        ids: vec!["r".into()],
    });
    assert_eq!(a.state().pointer.hover.as_deref(), Some("a"));
}

#[test]
fn a_node_deleted_during_a_drag_ends_it_and_the_release_is_not_refused() {
    let mut a = app(true);
    press(&mut a, 435.0, 35.0);
    moved(&mut a, 445.0, 45.0);
    assert!(matches!(
        a.state().pointer.gesture,
        PointerGesture::Move { .. }
    ));
    a.dispatch(Action::Delete {
        ids: vec!["fg".into()],
    });
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    a.take_events();
    assert!(release(&mut a, 445.0, 45.0).is_empty());
    assert!(refusals(&mut a).is_empty());
}

#[test]
fn a_zoom_around_the_pointer_in_the_middle_of_a_drag_leaves_the_node_under_it() {
    let mut a = app(true);
    press(&mut a, 435.0, 35.0);
    moved(&mut a, 445.0, 45.0);
    camera(
        &mut a,
        CameraOp::ZoomAt {
            factor: 2.0,
            at: Point { x: 445.0, y: 45.0 },
        },
    );
    let c = a.state().camera;
    let (sx, sy) = (445.0 * c.zoom + c.x, 45.0 * c.zoom + c.y);
    assert_eq!(
        (sx, sy),
        (445.0, 45.0),
        "the world point under the pointer stays"
    );
    release(&mut a, sx, sy);
    let n = node(&a, "fg");
    assert_eq!(
        (n.x, n.y),
        (30.0, 30.0),
        "moved by the 10 units dragged, not by what the zoom made of them"
    );
}

#[test]
fn a_cancel_at_a_point_that_is_not_in_the_world_ends_the_drag() {
    let mut a = app(true);
    camera(
        &mut a,
        CameraOp::ZoomAt {
            factor: 0.02,
            at: Point { x: 0.0, y: 0.0 },
        },
    );
    press(&mut a, 8.7, 0.7);
    moved(&mut a, 15.0, 0.7);
    assert!(matches!(
        a.state().pointer.gesture,
        PointerGesture::Move { .. }
    ));
    send(&mut a, input(PointerPhase::Cancel, 1e308, 1e308));
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    assert!(release(&mut a, 15.0, 0.7).is_empty());
    assert_eq!(steps(&a), 0);
}
