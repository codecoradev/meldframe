//! The pointer that sizes, draws and sweeps, on a laid-out canvas with no front end (E8.1.4, part 2): a resize by a handle of the
//! selection box, a node drawn with a tool, and a marquee, each as one undo step with its preview, read from `AppState` and
//! `App::pointer_preview`. The geometry is the rules of `handle` and `draw`; these tests hold the machine to them.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::pointer::*;
use layout_fixture::*;
use meldframe_app::{
    Action, Modifiers, PointerCursor, PointerGesture, PointerInput, PointerPhase, PointerPreview,
    Tool,
};
use meldframe_protocol::{Command, SizeMode};

fn shift(phase: PointerPhase, x: f64, y: f64) -> PointerInput {
    PointerInput {
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
        ..input(phase, x, y)
    }
}

// --- resize by a handle --------------------------------------------------------------------------------------------

#[test]
fn a_handle_of_the_selection_box_resizes_as_one_undo_step_and_writes_nothing_until_it_is_let_go() {
    let mut a = app(true);
    click(&mut a, 435.0, 35.0); // `fg`, a free rectangle at (420, 20), 30 x 30
    assert_eq!(selection(&a), ["fg"]);
    moved(&mut a, 450.0, 50.0); // the south east handle
    assert_eq!(a.state().pointer.cursor, PointerCursor::NwseResize);
    assert_eq!(a.state().pointer.hover, None, "the handle is not the node");
    press(&mut a, 450.0, 50.0);
    assert_eq!(selection(&a), ["fg"], "a press on a handle selects nothing");
    moved(&mut a, 451.0, 50.0);
    assert_eq!(
        a.state().pointer.gesture,
        PointerGesture::Idle,
        "inside the threshold"
    );
    moved(&mut a, 455.0, 57.0);
    let Some(PointerPreview::Resize(p)) = a.pointer_preview() else {
        panic!("a resize has a preview")
    };
    assert_eq!((p.world.w, p.world.h), (35.0, 37.0));
    assert_eq!(
        (node(&a, "fg").w, node(&a, "fg").h),
        (30.0, 30.0),
        "the file is as it was"
    );
    assert_eq!(steps(&a), 0);
    assert_eq!(a.state().pointer.cursor, PointerCursor::NwseResize);
    let sent = release(&mut a, 455.0, 57.0);
    assert_eq!(sent.len(), 1, "one op: {sent:?}");
    let n = node(&a, "fg");
    assert_eq!((n.x, n.y, n.w, n.h), (20.0, 20.0, 35.0, 37.0));
    assert_eq!(steps(&a), 1);
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    a.dispatch(Action::Undo {});
    assert_eq!((node(&a, "fg").w, node(&a, "fg").h), (30.0, 30.0));
}

#[test]
fn a_flow_childs_fill_side_that_gets_a_size_becomes_fixed_and_no_origin_is_written() {
    let mut a = app_of(with_sized_b("fill"), true);
    click(&mut a, 30.0, 50.0); // `b`, a flow child in `f` that fills its width: drawn at (10, 35), 180 wide and 30 high
    // The east handle sits at (190, 50) on the box the canvas draws.
    press(&mut a, 190.0, 50.0);
    moved(&mut a, 180.0, 50.0);
    let sent = release(&mut a, 180.0, 50.0);
    assert_eq!(sent.len(), 1, "{sent:?}");
    let b = node(&a, "b");
    assert_eq!(
        (b.x, b.w),
        (0.0, 170.0),
        "the width is the drawn one less the drag; the flow places it"
    );
    assert_eq!(
        b.sw,
        SizeMode::Fixed,
        "a fill side that gets a size is fixed"
    );
    assert_eq!(steps(&a), 1);
}

#[test]
fn the_handles_of_several_nodes_do_nothing() {
    let mut a = app(true);
    a.dispatch(Action::Select {
        ids: vec!["fg".into(), "g".into()],
    });
    // The box of both is (400, 0) to (600, 200): its south east handle is at (600, 200).
    press(&mut a, 600.0, 200.0);
    moved(&mut a, 620.0, 220.0);
    assert_eq!(
        a.state().pointer.gesture,
        PointerGesture::Idle,
        "one node at a time"
    );
    assert!(release(&mut a, 620.0, 220.0).is_empty());
    assert_eq!(selection(&a), ["fg", "g"], "and the press selected nothing");
    assert_eq!(steps(&a), 0);
}

#[test]
fn a_halted_app_begins_no_resize_and_no_draw() {
    let mut a = app(true);
    click(&mut a, 435.0, 35.0);
    halt(&mut a);
    let before = serde_json::to_value(a.nodes()).unwrap();
    press(&mut a, 450.0, 50.0);
    moved(&mut a, 480.0, 90.0);
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    release(&mut a, 480.0, 90.0);
    a.dispatch(Action::SetTool {
        tool: Tool::Rectangle,
    });
    press(&mut a, 900.0, 500.0);
    moved(&mut a, 960.0, 540.0);
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    assert!(release(&mut a, 960.0, 540.0).is_empty());
    assert_eq!(serde_json::to_value(a.nodes()).unwrap(), before);
}

#[test]
fn a_new_selection_shows_its_handles_cursor_without_a_move() {
    let mut a = app(true);
    moved(&mut a, 450.0, 50.0); // over where the south east handle of `fg` will be
    assert_eq!(a.state().pointer.cursor, PointerCursor::Default);
    a.dispatch(Action::Select {
        ids: vec!["fg".into()],
    });
    assert_eq!(a.state().pointer.cursor, PointerCursor::NwseResize);
    a.dispatch(Action::ClearSelection {});
    assert_eq!(a.state().pointer.cursor, PointerCursor::Default);
}

#[test]
fn a_node_deleted_during_a_resize_ends_it_and_the_release_is_not_refused() {
    let mut a = app(true);
    click(&mut a, 435.0, 35.0);
    press(&mut a, 450.0, 50.0);
    moved(&mut a, 470.0, 70.0);
    assert!(matches!(
        a.state().pointer.gesture,
        PointerGesture::Resize { .. }
    ));
    a.dispatch(Action::Delete {
        ids: vec!["fg".into()],
    });
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    a.take_events();
    assert!(release(&mut a, 470.0, 70.0).is_empty());
    assert!(refusals(&mut a).is_empty());
}

#[test]
fn a_cancel_during_a_resize_leaves_the_node_as_it_was() {
    let mut a = app(true);
    click(&mut a, 435.0, 35.0);
    press(&mut a, 450.0, 50.0);
    moved(&mut a, 480.0, 90.0);
    assert!(matches!(
        a.state().pointer.gesture,
        PointerGesture::Resize { .. }
    ));
    send(&mut a, input(PointerPhase::Cancel, 480.0, 90.0));
    assert_eq!(a.state().pointer.gesture, PointerGesture::Idle);
    assert!(release(&mut a, 480.0, 90.0).is_empty());
    assert_eq!((node(&a, "fg").w, node(&a, "fg").h), (30.0, 30.0));
    assert_eq!(steps(&a), 0);
}

// --- draw with a tool ----------------------------------------------------------------------------------------------

#[test]
fn a_drag_with_a_tool_draws_a_node_as_one_undo_step_and_goes_back_to_select() {
    let mut a = app(true);
    a.dispatch(Action::SetTool {
        tool: Tool::Rectangle,
    });
    press(&mut a, 900.0, 500.0);
    assert_eq!(a.state().pointer.cursor, PointerCursor::Crosshair);
    moved(&mut a, 940.0, 520.0);
    let Some(PointerPreview::Draw(p)) = a.pointer_preview() else {
        panic!("a draw has a preview")
    };
    assert_eq!(
        (p.world.x, p.world.y, p.world.w, p.world.h),
        (900.0, 500.0, 40.0, 20.0)
    );
    assert_eq!(a.nodes().len(), 10, "nothing is made yet");
    let sent = release(&mut a, 940.0, 520.0);
    assert!(
        matches!(sent.as_slice(), [Command::AddNode { .. }]),
        "{sent:?}"
    );
    assert_eq!(a.nodes().len(), 11);
    assert_eq!(a.state().tool, Tool::Select);
    assert_eq!(selection(&a).len(), 1);
    let n = node(&a, &selection(&a)[0]);
    assert_eq!((n.x, n.y, n.w, n.h), (900.0, 500.0, 40.0, 20.0));
    assert_eq!(steps(&a), 1);
    a.dispatch(Action::Undo {});
    assert_eq!(a.nodes().len(), 10);
}

#[test]
fn a_drag_up_and_left_draws_the_same_box_and_a_click_makes_the_default_size_in_the_frame_under_it()
{
    let mut a = app(true);
    a.dispatch(Action::SetTool {
        tool: Tool::Ellipse,
    });
    press(&mut a, 940.0, 520.0);
    moved(&mut a, 900.0, 500.0);
    release(&mut a, 900.0, 500.0);
    let n = node(&a, &selection(&a)[0]);
    assert_eq!((n.x, n.y, n.w, n.h), (900.0, 500.0, 40.0, 20.0));
    // A click inside the free frame `g` (at x 400): the node goes into it, at its own offset, with the default size.
    a.dispatch(Action::SetTool { tool: Tool::Frame });
    click(&mut a, 500.0, 100.0);
    let made = node(&a, &selection(&a)[0]);
    assert_eq!(made.parent_id.as_deref(), Some("g"));
    assert_eq!((made.x, made.y), (100.0, 100.0));
    assert!(made.w > 1.0 && made.h > 1.0, "the default size of a frame");
    assert_eq!(steps(&a), 2);
}

#[test]
fn a_cancel_while_drawing_makes_nothing_and_a_halted_app_begins_nothing() {
    let mut a = app(true);
    a.dispatch(Action::SetTool { tool: Tool::Text });
    press(&mut a, 900.0, 500.0);
    moved(&mut a, 960.0, 540.0);
    assert!(matches!(
        a.state().pointer.gesture,
        PointerGesture::Draw { .. }
    ));
    send(&mut a, input(PointerPhase::Leave, 960.0, 540.0));
    assert!(release(&mut a, 960.0, 540.0).is_empty());
    assert_eq!(a.nodes().len(), 10);
    assert_eq!(a.state().tool, Tool::Text, "the tool stays");
}

// --- marquee -------------------------------------------------------------------------------------------------------

#[test]
fn a_marquee_selects_the_top_level_nodes_it_overlaps_and_shows_a_normalised_rectangle() {
    let mut a = app(true);
    press(&mut a, 250.0, 150.0);
    moved(&mut a, 100.0, 50.0);
    assert!(
        selection(&a).is_empty(),
        "nothing selected until it is let go"
    );
    let Some(PointerPreview::Marquee { rect }) = a.pointer_preview() else {
        panic!("a marquee has a preview")
    };
    assert_eq!(
        (rect.x, rect.y, rect.w, rect.h),
        (100.0, 50.0, 150.0, 100.0)
    );
    moved(&mut a, -50.0, -50.0);
    release(&mut a, -50.0, -50.0);
    assert_eq!(
        selection(&a),
        ["f"],
        "the frame, not `a`, `b` and `c` inside it"
    );
    assert_eq!(steps(&a), 0);
    // Wider: the frames in paint order. A marquee that meets nothing clears.
    press(&mut a, -50.0, -50.0);
    moved(&mut a, 1100.0, 400.0);
    release(&mut a, 1100.0, 400.0);
    assert_eq!(selection(&a), ["f", "g", "r"]);
    press(&mut a, -50.0, -50.0);
    moved(&mut a, -10.0, -10.0);
    release(&mut a, -10.0, -10.0);
    assert!(selection(&a).is_empty());
}

#[test]
fn shift_adds_a_marquee_to_the_selection_and_keeps_it_when_nothing_is_added() {
    let mut a = app(true);
    click(&mut a, 435.0, 35.0);
    send(&mut a, shift(PointerPhase::Down, -50.0, -50.0));
    send(&mut a, shift(PointerPhase::Move, 250.0, 150.0));
    send(&mut a, shift(PointerPhase::Up, 250.0, 150.0));
    assert_eq!(selection(&a), ["fg", "f"]);
    send(&mut a, shift(PointerPhase::Down, -50.0, -50.0));
    send(&mut a, shift(PointerPhase::Move, -10.0, -10.0));
    send(&mut a, shift(PointerPhase::Up, -10.0, -10.0));
    assert_eq!(selection(&a), ["fg", "f"]);
}

#[test]
fn a_cancel_during_a_marquee_leaves_the_selection_as_it_was() {
    let mut a = app(true);
    click(&mut a, 435.0, 35.0);
    press(&mut a, -50.0, -50.0);
    moved(&mut a, 250.0, 150.0);
    send(&mut a, input(PointerPhase::Cancel, 250.0, 150.0));
    release(&mut a, 250.0, 150.0);
    assert_eq!(selection(&a), ["fg"]);
}
