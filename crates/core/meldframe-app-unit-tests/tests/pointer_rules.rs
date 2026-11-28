//! The pointer machine of `meldframe_app::pointer_machine` against a made-up canvas, ported from
//! `packages/web-canvas/src/interaction.test.ts` with the same cases and numbers: two nodes in world space, `a` at (0, 0) and `b`
//! at (200, 0), both 100 by 100. The machine reads the app through `Env` and answers with actions; the rig applies the
//! selection actions as the app does, so the next event sees them.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod pointer_rig;
use meldframe_app::pointer_machine::DRAG_THRESHOLD;
use meldframe_app_model::{
    Action, Camera, Modifiers, Point, PointerButton, PointerCursor, PointerGesture, PointerInput,
    PointerPhase, Tool,
};
use pointer_rig::*;

// --- selection clicks ----------------------------------------------------------------------------------------------

#[test]
fn a_click_on_a_node_selects_it() {
    let mut r = rig(home(), &[]);
    r.down(50.0, 50.0);
    r.up(50.0, 50.0);
    assert_eq!(r.json(), [select(&["a"])]);
}

#[test]
fn a_click_on_empty_canvas_clears_the_selection() {
    let mut r = rig(home(), &["a"]);
    r.down(500.0, 500.0);
    r.up(500.0, 500.0);
    assert_eq!(r.json(), [select(&[])]);
}

#[test]
fn shift_adds_without_clearing_also_on_empty_canvas() {
    let mut r = rig(home(), &["a"]);
    r.send(PointerPhase::Down, 250.0, 50.0, true);
    r.up(250.0, 50.0);
    assert_eq!(
        r.json(),
        [serde_json::json!({"type": "toggleSelect", "id": "b"})]
    );
    let mut e = rig(home(), &["a"]);
    e.send(PointerPhase::Down, 500.0, 500.0, true);
    e.up(500.0, 500.0);
    assert!(e.json().is_empty());
}

#[test]
fn clicking_one_of_several_selected_nodes_narrows_on_release_but_not_when_it_was_dragged() {
    let mut r = rig(home(), &["a", "b"]);
    r.down(50.0, 50.0);
    assert!(r.log.is_empty());
    r.up(50.0, 50.0);
    assert_eq!(r.json(), [select(&["a"])]);
    let mut d = rig(home(), &["a", "b"]);
    d.down(50.0, 50.0);
    d.mv(80.0, 50.0);
    d.up(80.0, 50.0);
    assert!(
        d.log
            .iter()
            .all(|a| !matches!(a, Action::Select { .. } | Action::ToggleSelect { .. }))
    );
}

#[test]
fn shift_click_on_a_selected_node_toggles_it_out() {
    let mut r = rig(home(), &["a"]);
    r.send(PointerPhase::Down, 50.0, 50.0, true);
    r.up(50.0, 50.0);
    assert_eq!(
        r.json(),
        [serde_json::json!({"type": "toggleSelect", "id": "a"})]
    );
}

#[test]
fn other_buttons_do_nothing() {
    let mut r = rig(home(), &[]);
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        r.event(PointerInput {
            phase,
            x: 50.0,
            y: 50.0,
            modifiers: Modifiers::default(),
            button: PointerButton::Secondary,
            touch: false,
            space: false,
            at_ms: None,
        });
    }
    assert!(r.log.is_empty());
}

// --- move ----------------------------------------------------------------------------------------------------------

fn gesture(r: &Rig) -> (Vec<String>, Point, Point) {
    match r.machine.view().gesture.clone() {
        PointerGesture::Move { ids, at, by } => (ids, at, by),
        g => panic!("a move was expected, not {g:?}"),
    }
}

#[test]
fn a_drag_selects_the_node_then_reports_world_deltas_and_drops_on_release() {
    let mut r = rig(
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 2.0,
        },
        &[],
    );
    r.down(50.0, 50.0);
    r.mv(52.0, 50.0); // below the threshold: nothing yet
    assert_eq!(r.json(), [select(&["a"])]);
    assert_eq!(r.machine.view().gesture, PointerGesture::Idle);
    r.mv(70.0, 90.0);
    let (ids, at, by) = gesture(&r);
    assert_eq!(
        (ids, at.x, at.y, by.x, by.y),
        (vec!["a".into()], 35.0, 45.0, 10.0, 20.0)
    );
    r.up(70.0, 90.0);
    assert_eq!(
        r.json()[1],
        serde_json::json!({"type": "dropNodes", "ids": ["a"], "x": 35.0, "y": 45.0, "dx": 10.0, "dy": 20.0})
    );
}

#[test]
fn deltas_are_totals_since_the_press_so_a_host_that_applies_them_gets_no_drift() {
    let mut r = rig(home(), &[]);
    r.down(10.0, 10.0);
    r.mv(20.0, 10.0);
    let (_, _, by) = gesture(&r);
    assert_eq!((by.x, by.y), (10.0, 0.0));
    r.mv(40.0, 30.0);
    let (_, _, by) = gesture(&r);
    assert_eq!((by.x, by.y), (30.0, 20.0));
}

#[test]
fn a_drag_of_a_selected_node_moves_the_whole_selection() {
    let mut r = rig(home(), &["a", "b"]);
    r.down(50.0, 50.0);
    r.mv(60.0, 50.0);
    assert_eq!(gesture(&r).0, ["a", "b"]);
}

#[test]
fn a_node_is_not_listed_twice_when_the_press_already_added_it_to_the_selection() {
    let mut r = rig(home(), &["b"]);
    r.send(PointerPhase::Down, 50.0, 50.0, true);
    assert_eq!(r.selection, ["b", "a"], "the press added it");
    r.send(PointerPhase::Move, 80.0, 50.0, true);
    assert_eq!(gesture(&r).0, ["b", "a"]);
}

#[test]
fn a_cancelled_pointer_ends_the_gesture_and_applies_nothing() {
    let mut r = rig(home(), &[]);
    r.down(50.0, 50.0);
    r.mv(90.0, 50.0);
    r.send(PointerPhase::Cancel, 90.0, 50.0, false);
    assert_eq!(r.machine.view().gesture, PointerGesture::Idle);
    r.up(90.0, 50.0);
    assert_eq!(
        r.json(),
        [select(&["a"])],
        "only the selection of the press"
    );
}

#[test]
fn the_threshold_is_in_screen_pixels() {
    let mut r = rig(
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 0.1,
        },
        &[],
    );
    r.down(5.0, 5.0);
    r.mv(5.0 + DRAG_THRESHOLD - 1.0, 5.0);
    assert_eq!(r.machine.view().gesture, PointerGesture::Idle);
    r.mv(5.0 + DRAG_THRESHOLD + 1.0, 5.0);
    assert!(matches!(
        r.machine.view().gesture,
        PointerGesture::Move { .. }
    ));
}

#[test]
fn the_pointer_is_in_world_units_when_the_camera_is_zoomed_and_panned() {
    let mut r = rig(
        Camera {
            x: 100.0,
            y: 20.0,
            zoom: 2.0,
        },
        &[],
    );
    r.down(150.0, 70.0); // world (25, 25), on node a
    r.mv(200.0, 120.0);
    r.mv(220.0, 140.0);
    let (_, at, by) = gesture(&r);
    assert_eq!((at.x, at.y, by.x, by.y), (60.0, 60.0, 35.0, 35.0));
}

// --- cursor and hover ----------------------------------------------------------------------------------------------

#[test]
fn the_cursor_follows_the_tool_space_and_a_pan() {
    let mut r = rig(home(), &[]);
    r.mv(500.0, 500.0);
    assert_eq!(r.machine.view().cursor, PointerCursor::Default);
    r.tool = Tool::Rectangle;
    r.mv(500.0, 500.0);
    assert_eq!(r.machine.view().cursor, PointerCursor::Crosshair);
    r.tool = Tool::Select;
    r.event(PointerInput {
        phase: PointerPhase::Move,
        x: 500.0,
        y: 500.0,
        modifiers: Modifiers::default(),
        button: PointerButton::Primary,
        touch: false,
        space: true,
        at_ms: None,
    });
    assert_eq!(r.machine.view().cursor, PointerCursor::Grab);
}

#[test]
fn hover_clears_when_the_pointer_leaves_and_is_not_asked_while_a_button_is_held() {
    let mut r = rig(home(), &[]);
    r.mv(50.0, 50.0);
    assert_eq!(r.machine.view().hover.as_deref(), Some("a"));
    r.send(PointerPhase::Leave, 50.0, 50.0, false);
    assert_eq!(r.machine.view().hover, None);
    r.down(250.0, 50.0);
    r.mv(500.0, 500.0);
    assert_eq!(r.machine.view().hover, None, "a drag has no hover");
}

#[test]
fn a_click_on_the_one_selected_node_and_a_drag_that_comes_back_make_no_action() {
    let mut r = rig(home(), &["a"]);
    r.down(50.0, 50.0);
    r.up(50.0, 50.0);
    assert!(r.log.is_empty(), "nothing to narrow");
    r.down(50.0, 50.0);
    r.mv(90.0, 50.0);
    assert!(matches!(
        r.machine.view().gesture,
        PointerGesture::Move { .. }
    ));
    r.mv(50.0, 50.0);
    r.up(50.0, 50.0);
    assert!(r.log.is_empty(), "no travel, no drop");
}

// --- what the front end does while a gesture goes on ---------------------------------------------------------------

#[test]
fn a_zoom_around_the_pointer_in_the_middle_of_a_drag_does_not_make_the_node_jump() {
    let mut r = rig(home(), &[]);
    r.down(50.0, 50.0);
    r.mv(80.0, 50.0);
    assert_eq!(gesture(&r).2, Point { x: 30.0, y: 0.0 });
    // Ctrl+wheel at the pointer: the world point under it stays under it.
    r.camera = Camera {
        x: -80.0,
        y: -50.0,
        zoom: 2.0,
    };
    r.mv(80.0, 50.0);
    let (_, at, by) = gesture(&r);
    assert_eq!((at.x, at.y), (80.0, 50.0));
    assert_eq!(
        (by.x, by.y),
        (30.0, 0.0),
        "the travel is in the world, from where the press was"
    );
}

#[test]
fn a_cancel_or_a_leave_ends_the_gesture_whatever_its_point_is() {
    for phase in [PointerPhase::Cancel, PointerPhase::Leave] {
        let mut r = rig(
            Camera {
                x: 0.0,
                y: 0.0,
                zoom: 0.02,
            },
            &[],
        );
        r.down(1.0, 1.0);
        r.mv(30.0, 1.0);
        assert!(matches!(
            r.machine.view().gesture,
            PointerGesture::Move { .. }
        ));
        // A point so far out that its world point is not a number: the point is not read, so the gesture ends all the same.
        r.send(phase, 1e308, 1e308, false);
        assert_eq!(r.machine.view().gesture, PointerGesture::Idle, "{phase:?}");
        r.up(30.0, 1.0);
        assert_eq!(
            r.json(),
            [select(&["a"])].to_vec(),
            "only the press selected"
        );
    }
}

#[test]
fn the_cursor_and_the_hover_can_be_shown_again_at_the_last_point() {
    let mut r = rig(home(), &[]);
    assert!(r.machine.refresh().is_none(), "no point yet");
    r.mv(50.0, 50.0);
    r.tool = Tool::Rectangle;
    let again = r.machine.refresh().expect("a point is known");
    r.event(again);
    assert_eq!(r.machine.view().cursor, PointerCursor::Crosshair);
    assert_eq!(r.machine.view().hover, None, "a drawing tool has no hover");
    r.tool = Tool::Select;
    let again = r.machine.refresh().expect("a point is known");
    r.event(again);
    assert_eq!(r.machine.view().hover.as_deref(), Some("a"));
    // Not while a gesture goes on, and not after the pointer left.
    r.down(50.0, 50.0);
    r.mv(90.0, 50.0);
    assert!(r.machine.refresh().is_none());
    r.send(PointerPhase::Leave, 90.0, 50.0, false);
    assert!(r.machine.refresh().is_none());
}

#[test]
fn a_node_that_goes_leaves_the_hover_and_ends_a_drag_that_holds_it() {
    let mut r = rig(home(), &["a", "b"]);
    r.mv(50.0, 50.0);
    assert!(!r.machine.forget(&|_| true), "nothing went");
    assert!(r.machine.forget(&|id| id != "a"));
    assert_eq!(r.machine.view().hover, None);
    r.down(50.0, 50.0);
    r.mv(90.0, 50.0);
    assert!(
        !r.machine.forget(&|id| id != "x"),
        "not one of the dragged nodes"
    );
    assert!(
        r.machine.forget(&|id| id != "b"),
        "one of the dragged nodes"
    );
    assert_eq!(r.machine.view().gesture, PointerGesture::Idle);
    r.up(90.0, 50.0);
    assert!(
        r.log.iter().all(|a| !matches!(a, Action::DropNodes { .. })),
        "what is let go would be refused, so nothing is sent"
    );
}
