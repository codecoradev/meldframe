//! The pointer on a laid-out canvas with no front end (E8.1.4, part 2): select by click, move by drag as one undo step and a drop
//! above a sibling that reorders with its insertion line, all read from `AppState` and `App::pointer_preview`. `App::hit` is the only
//! rule for what is under the pointer: the tests never ask a box. Pan, hover, cancel and the refusals are `layout_pointer_view.rs`.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::pointer::*;
use layout_fixture::*;
use meldframe_app::{
    Action, CameraOp, Modifiers, Point, PointerButton, PointerGesture, PointerInput, PointerPhase,
    PointerPreview,
};
use meldframe_protocol::Command;

// --- select by click -----------------------------------------------------------------------------------------------

#[test]
fn a_click_selects_the_node_under_the_pointer_and_a_click_on_nothing_clears() {
    let mut a = app(true);
    click(&mut a, 30.0, 20.0);
    assert_eq!(selection(&a), ["a"]);
    click(&mut a, 900.0, 500.0);
    assert!(selection(&a).is_empty());
    assert_eq!(steps(&a), 0, "a selection is not an edit");
}

#[test]
fn a_press_selects_at_once_and_shift_adds_and_takes_out() {
    let mut a = app(true);
    press(&mut a, 30.0, 20.0);
    assert_eq!(selection(&a), ["a"], "on the press, not on the release");
    release(&mut a, 30.0, 20.0);
    let shift = |phase, x, y| PointerInput {
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
        ..input(phase, x, y)
    };
    send(&mut a, shift(PointerPhase::Down, 430.0, 40.0));
    send(&mut a, shift(PointerPhase::Up, 430.0, 40.0));
    assert_eq!(selection(&a), ["a", "fg"]);
    // Shift on a selected node takes it out, on the release.
    send(&mut a, shift(PointerPhase::Down, 30.0, 20.0));
    assert_eq!(selection(&a), ["a", "fg"]);
    send(&mut a, shift(PointerPhase::Up, 30.0, 20.0));
    assert_eq!(selection(&a), ["fg"]);
    // Shift on empty canvas keeps the selection.
    send(&mut a, shift(PointerPhase::Down, 900.0, 500.0));
    send(&mut a, shift(PointerPhase::Up, 900.0, 500.0));
    assert_eq!(selection(&a), ["fg"]);
}

#[test]
fn a_click_on_one_of_several_selected_nodes_narrows_to_it_on_release_but_not_after_a_drag() {
    let mut a = app(true);
    a.dispatch(Action::Select {
        ids: vec!["a".into(), "fg".into()],
    });
    press(&mut a, 30.0, 20.0);
    assert_eq!(selection(&a), ["a", "fg"]);
    release(&mut a, 30.0, 20.0);
    assert_eq!(selection(&a), ["a"]);
    a.dispatch(Action::Select {
        ids: vec!["a".into(), "fg".into()],
    });
    press(&mut a, 430.0, 40.0);
    moved(&mut a, 440.0, 40.0);
    release(&mut a, 440.0, 40.0);
    assert_eq!(
        selection(&a),
        ["a", "fg"].map(String::from),
        "dragged, so kept"
    );
}

#[test]
fn the_secondary_button_selects_nothing() {
    let mut a = app(true);
    let secondary = PointerInput {
        button: PointerButton::Secondary,
        ..input(PointerPhase::Down, 30.0, 20.0)
    };
    send(&mut a, secondary);
    assert!(selection(&a).is_empty());
}

// --- move by drag --------------------------------------------------------------------------------------------------

#[test]
fn a_drag_moves_the_node_as_one_undo_step_and_writes_nothing_until_it_is_let_go() {
    let mut a = app(true);
    let before = node(&a, "fg");
    press(&mut a, 435.0, 35.0);
    a.take_events();
    for (x, y) in [(437.0, 35.0), (440.0, 40.0), (445.0, 45.0)] {
        let fx = send(&mut a, input(PointerPhase::Move, x, y));
        assert!(fx.is_empty(), "no op while the pointer is down");
    }
    assert_eq!(
        (node(&a, "fg").x, node(&a, "fg").y),
        (before.x, before.y),
        "the file is as it was"
    );
    assert_eq!(steps(&a), 0);
    let PointerGesture::Move { ids, at, by } = a.state().pointer.gesture.clone() else {
        panic!("a move is going on")
    };
    assert_eq!(
        (ids, at.x, at.y, by.x, by.y),
        (vec!["fg".to_string()], 445.0, 45.0, 10.0, 10.0)
    );
    a.take_events();
    let sent = release(&mut a, 445.0, 45.0);
    assert_eq!(sent.len(), 1, "one op: {sent:?}");
    let now = node(&a, "fg");
    assert_eq!((now.x, now.y), (30.0, 30.0));
    assert_eq!(steps(&a), 1);
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    assert_eq!(changed(&a.take_events()), 1, "one DocumentChanged");
    a.dispatch(Action::Undo {});
    let back = node(&a, "fg");
    assert_eq!(
        (back.x, back.y),
        (before.x, before.y),
        "one undo takes the whole drag back"
    );
}

#[test]
fn a_press_that_stays_inside_the_threshold_is_a_click_and_moves_nothing() {
    let mut a = app(true);
    press(&mut a, 435.0, 35.0);
    moved(&mut a, 437.0, 36.0);
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    assert!(release(&mut a, 437.0, 36.0).is_empty());
    assert_eq!(steps(&a), 0);
    assert_eq!(selection(&a), ["fg"]);
}

#[test]
fn the_drag_is_worked_out_in_world_units_through_the_camera() {
    let mut a = app(true);
    camera(
        &mut a,
        CameraOp::Pan {
            dx: 100.0,
            dy: 20.0,
        },
    );
    camera(
        &mut a,
        CameraOp::ZoomAt {
            factor: 2.0,
            at: Point { x: 0.0, y: 0.0 },
        },
    );
    let c = a.state().camera;
    let screen = |wx: f64, wy: f64| (wx * c.zoom + c.x, wy * c.zoom + c.y);
    let (px, py) = screen(435.0, 35.0);
    press(&mut a, px, py);
    assert_eq!(selection(&a), ["fg"]);
    let (qx, qy) = screen(445.0, 45.0);
    moved(&mut a, qx, qy);
    let PointerGesture::Move { at, by, .. } = a.state().pointer.gesture.clone() else {
        panic!("a move is going on")
    };
    assert_eq!((at.x, at.y, by.x, by.y), (445.0, 45.0, 10.0, 10.0));
    release(&mut a, qx, qy);
    assert_eq!((node(&a, "fg").x, node(&a, "fg").y), (30.0, 30.0));
}

#[test]
fn the_threshold_is_in_screen_pixels() {
    let mut a = app(true);
    camera(
        &mut a,
        CameraOp::ZoomAt {
            factor: 0.1,
            at: Point { x: 0.0, y: 0.0 },
        },
    );
    press(&mut a, 43.0, 3.5);
    assert_eq!(selection(&a), ["fg"]);
    moved(&mut a, 45.0, 3.5);
    assert_eq!(
        a.state().pointer.gesture,
        PointerGesture::Idle,
        "2 px is below 3"
    );
    moved(&mut a, 47.0, 3.5);
    assert!(matches!(
        a.state().pointer.gesture,
        PointerGesture::Move { .. }
    ));
}

// --- a drop in a flow ----------------------------------------------------------------------------------------------

#[test]
fn a_drop_above_a_sibling_reorders_and_the_preview_is_the_insertion_line_until_then() {
    let mut a = app(true);
    press(&mut a, 30.0, 50.0); // `b`, drawn from y 35 to 65
    moved(&mut a, 30.0, 40.0);
    moved(&mut a, 30.0, 15.0); // above the middle of `a`
    let Some(PointerPreview::Move {
        slot,
        follow,
        dx,
        dy,
    }) = a.pointer_preview()
    else {
        panic!("a move has a preview")
    };
    let slot = slot.expect("a slot in the flow of `f`");
    assert_eq!((slot.frame.as_str(), slot.index), ("f", 0));
    assert_eq!((dx, dy), (0.0, -35.0));
    assert!(
        follow.is_empty(),
        "a flow child stays where it is drawn while the line shows"
    );
    assert_eq!(
        kids(&a, Some("f")),
        ["a", "b", "c", "abs"],
        "nothing written yet"
    );
    let sent = release(&mut a, 30.0, 15.0);
    assert!(matches!(
        sent.as_slice(),
        [Command::Reorder { id, before: Some(first), .. }] if id == "b" && first == "a"
    ));
    assert_eq!(kids(&a, Some("f")), ["b", "a", "c", "abs"]);
    assert_eq!(steps(&a), 1);
    assert!(
        a.pointer_preview().is_none(),
        "nothing is going on any more"
    );
    a.dispatch(Action::Undo {});
    assert_eq!(kids(&a, Some("f")), ["a", "b", "c", "abs"]);
}

#[test]
fn with_the_layout_off_the_nodes_follow_the_pointer_and_the_drop_is_a_move_by_the_drag() {
    let mut a = app(false);
    press(&mut a, 435.0, 35.0);
    moved(&mut a, 445.0, 45.0);
    let Some(PointerPreview::Move { slot, follow, .. }) = a.pointer_preview() else {
        panic!("a move has a preview")
    };
    assert!(slot.is_none());
    assert_eq!(follow, ["fg"]);
    release(&mut a, 445.0, 45.0);
    assert_eq!((node(&a, "fg").x, node(&a, "fg").y), (30.0, 30.0));
}
