//! A drop in a laid-out frame (the host flag `layout_on_canvas` on): a child of an auto layout frame is reordered in its
//! flow or taken to another frame, a node of a free frame is adopted by the flow it is dropped on, and anything else
//! moves. With the flag off a drop is the plain move it always was.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::Action;
use meldframe_protocol::{Command, SizeMode};
use serde_json::json;

// --- reorder inside the frame -------------------------------------------------------------------------------------

#[test]
fn a_drop_above_the_first_child_puts_it_first_and_leaves_its_stored_position() {
    let mut a = app(true);
    let before = node(&a, "b");
    // `a` is drawn from y 10 to 30, its middle is 20: the pointer is above it.
    let sent = drop_node(&mut a, "b", (30.0, 15.0), (0.0, -20.0));
    assert!(matches!(
        sent.as_slice(),
        [Command::Reorder { id, before: Some(b), after: None, pos: None }] if id == "b" && b == "a"
    ));
    assert_eq!(kids(&a, Some("f")), ["b", "a", "c", "abs"]);
    let now = node(&a, "b");
    assert_eq!(
        (now.x, now.y),
        (before.x, before.y),
        "the layout ignores them"
    );
    assert_eq!(box_of(&a, "b"), (10.0, 10.0, 50.0, 30.0), "drawn first");
    assert_eq!(box_of(&a, "a"), (10.0, 45.0, 50.0, 20.0));
    assert_eq!(steps(&a), 1);
}

#[test]
fn a_drop_between_two_children_and_below_the_last_one_use_the_middles_of_the_others() {
    let mut a = app(true);
    // `b` (middle 50) is above the pointer and `c` (middle 90) below it: between them.
    drop_node(&mut a, "a", (30.0, 60.0), (0.0, 0.0));
    assert_eq!(kids(&a, Some("f")), ["b", "a", "c", "abs"]);
    // Below every other flow child (the absolute one is not one of them).
    drop_node(&mut a, "b", (30.0, 280.0), (0.0, 0.0));
    assert_eq!(kids(&a, Some("f")), ["a", "c", "b", "abs"]);
    assert_eq!(steps(&a), 2);
}

#[test]
fn a_drop_where_it_already_is_changes_nothing() {
    let mut a = app(true);
    let sent = drop_node(&mut a, "b", (30.0, 50.0), (0.0, 0.0));
    assert!(sent.is_empty());
    assert_eq!(steps(&a), 0);
    assert!(refusals(&mut a).is_empty());
}

#[test]
fn in_a_row_the_main_axis_is_x() {
    let mut a = app(true);
    // `r1` is drawn from x 716 to 756, its middle is 736.
    drop_node(&mut a, "r2", (710.0, 50.0), (0.0, 0.0));
    assert_eq!(kids(&a, Some("r")), ["r2", "r1"]);
    assert_eq!(box_of(&a, "r2"), (16.0, 16.0, 40.0, 40.0));
}

#[test]
fn one_undo_step_puts_the_order_back() {
    let mut a = app(true);
    drop_node(&mut a, "c", (30.0, 12.0), (0.0, 0.0));
    assert_eq!(kids(&a, Some("f")), ["c", "a", "b", "abs"]);
    a.dispatch(Action::Undo {});
    assert_eq!(kids(&a, Some("f")), ["a", "b", "c", "abs"]);
}

// --- another frame ------------------------------------------------------------------------------------------------

#[test]
fn a_drop_into_another_auto_layout_frame_adopts_the_node_at_the_drop_position() {
    let mut a = app(true);
    // The pointer is over `r`, left of the middle of `r1` (736): first.
    let sent = drop_node(&mut a, "b", (705.0, 50.0), (700.0, 0.0));
    assert!(matches!(
        sent.as_slice(),
        [Command::Reparent { id, parent_id: Some(p), pos: Some(_) }] if id == "b" && p == "r"
    ));
    assert_eq!(kids(&a, Some("r")), ["b", "r1", "r2"]);
    assert_eq!(kids(&a, Some("f")), ["a", "c", "abs"]);
    let b = node(&a, "b");
    assert_eq!((b.x, b.y), (0.0, 0.0), "stored position untouched");
    assert_eq!(box_of(&a, "b"), (16.0, 16.0, 50.0, 30.0));
    assert_eq!(steps(&a), 1);
    a.dispatch(Action::Undo {});
    assert_eq!(kids(&a, Some("f")), ["a", "b", "c", "abs"]);
}

#[test]
fn a_drop_between_the_children_of_another_frame_lands_between_them() {
    let mut a = app(true);
    // Right of the middle of `r1` (736), left of the middle of `r2` (786).
    drop_node(&mut a, "b", (745.0, 50.0), (0.0, 0.0));
    assert_eq!(kids(&a, Some("r")), ["r1", "b", "r2"]);
}

#[test]
fn a_drop_into_a_free_frame_keeps_the_place_it_was_dropped_at_and_a_fill_side_becomes_fixed() {
    let mut a = app_of(with_sized_b("fill"), true);
    assert_eq!(
        box_of(&a, "b"),
        (10.0, 35.0, 180.0, 30.0),
        "fills the width"
    );
    // Dragged 400 right and 100 down: its world box (10, 35) goes to (410, 135), which is (10, 135) in `g`.
    let sent = drop_node(&mut a, "b", (440.0, 150.0), (400.0, 100.0));
    assert!(matches!(
        sent.first(),
        Some(Command::Reparent { parent_id: Some(p), pos: None, .. }) if p == "g"
    ));
    let b = node(&a, "b");
    assert_eq!(b.parent_id.as_deref(), Some("g"));
    assert_eq!((b.x, b.y, b.w, b.sw), (10.0, 135.0, 180.0, SizeMode::Fixed));
    assert_eq!(box_of(&a, "b"), (10.0, 135.0, 180.0, 30.0));
    assert_eq!(steps(&a), 1, "one step for the whole drop");
    a.dispatch(Action::Undo {});
    let b = node(&a, "b");
    assert_eq!((b.parent_id.as_deref(), b.sw), (Some("f"), SizeMode::Fill));
}

#[test]
fn a_fill_height_becomes_fixed_too_when_the_node_leaves_the_flow() {
    let mut list = nodes();
    let b = list.iter_mut().find(|n| n.id == "b").unwrap();
    *b = serde_json::from_value(json!({
        "id": "b", "type": "rect", "parentId": "f", "pos": "a1", "w": 50, "h": 1, "sh": "fill"
    }))
    .unwrap();
    let mut a = app_of(list, true);
    // 300 high, padding 10 on both sides, `a` 20, `c` 40 and two gaps of 5 around `b`: 210 left to fill.
    assert_eq!(box_of(&a, "b"), (10.0, 35.0, 50.0, 210.0));
    drop_node(&mut a, "b", (1200.0, 600.0), (1000.0, 500.0));
    let b = node(&a, "b");
    assert_eq!(
        (b.sw, b.sh, b.w, b.h),
        (SizeMode::Fixed, SizeMode::Fixed, 50.0, 210.0)
    );
    assert_eq!((b.x, b.y), (1010.0, 535.0));
}

#[test]
fn a_drop_on_the_empty_page_takes_the_node_out_to_the_page() {
    let mut a = app(true);
    let sent = drop_node(&mut a, "c", (1200.0, 600.0), (1000.0, 500.0));
    assert!(matches!(
        sent.first(),
        Some(Command::Reparent {
            parent_id: None,
            ..
        })
    ));
    let c = node(&a, "c");
    assert_eq!(c.parent_id, None);
    assert_eq!((c.x, c.y), (1010.0, 570.0), "its world place plus the drag");
}

#[test]
fn a_node_of_a_free_frame_dropped_on_an_auto_layout_frame_is_adopted_by_its_flow() {
    let mut a = app(true);
    drop_node(&mut a, "fg", (30.0, 12.0), (-390.0, -20.0));
    assert_eq!(kids(&a, Some("f")), ["fg", "a", "b", "c", "abs"]);
    assert!(kids(&a, Some("g")).is_empty());
    let fg = node(&a, "fg");
    assert_eq!((fg.x, fg.y), (20.0, 20.0), "stored position untouched");
}

#[test]
fn a_node_of_a_free_frame_dropped_inside_its_own_frame_moves_by_the_drag() {
    let mut a = app(true);
    let sent = drop_node(&mut a, "fg", (480.0, 90.0), (30.0, 40.0));
    assert!(matches!(sent.as_slice(), [Command::UpdateNode { id, .. }] if id == "fg"));
    let fg = node(&a, "fg");
    assert_eq!(
        (fg.parent_id.as_deref(), fg.x, fg.y),
        (Some("g"), 50.0, 60.0)
    );
}

#[test]
fn a_frame_on_the_page_and_an_absolute_node_only_move() {
    let mut a = app(true);
    // The page frame `g` over `f`: it stays on the page.
    drop_node(&mut a, "g", (50.0, 50.0), (-350.0, 30.0));
    let g = node(&a, "g");
    assert_eq!((g.parent_id, g.x, g.y), (None, 50.0, 30.0));
    // The absolute child is a free node of its frame: dropped among the flow, it moves.
    drop_node(&mut a, "abs", (50.0, 40.0), (-10.0, -20.0));
    let abs = node(&a, "abs");
    assert_eq!(
        (abs.parent_id.as_deref(), abs.x, abs.y),
        (Some("f"), 140.0, 180.0)
    );
    assert_eq!(kids(&a, Some("f")), ["a", "b", "c", "abs"]);
}

#[test]
fn a_frame_dropped_over_a_sibling_frame_is_a_reorder_not_a_nest() {
    let mut a = app_of(cards(), true);
    // The middle of `c2` is x 270: the pointer is left of it, inside `c2` and inside `deck`.
    let sent = drop_node(&mut a, "c3", (260.0, 100.0), (-70.0, 0.0));
    assert!(matches!(
        sent.as_slice(),
        [Command::Reorder { id, before: Some(b), after: None, pos: None }] if id == "c3" && b == "c2"
    ));
    assert_eq!(kids(&a, Some("deck")), ["c1", "c3", "c2"]);
    assert_eq!(node(&a, "c3").parent_id.as_deref(), Some("deck"));
    assert_eq!(
        box_of(&a, "c3"),
        (120.0, 10.0, 100.0, 100.0),
        "second, in the space of the row"
    );
    assert_eq!(steps(&a), 1);
}

#[test]
fn the_pointer_on_the_middle_of_a_sibling_is_not_past_it() {
    let mut a = app_of(cards(), true);
    // Exactly the middle of `c2` (x 270): before it. Were the middle counted as past, `c3` would stay last.
    drop_node(&mut a, "c3", (270.0, 100.0), (0.0, 0.0));
    assert_eq!(kids(&a, Some("deck")), ["c1", "c3", "c2"]);
}

#[test]
fn a_frame_taken_out_of_its_row_onto_the_page_leaves_it() {
    let mut a = app_of(cards(), true);
    // Nothing is under (900, 700).
    let sent = drop_node(&mut a, "c3", (900.0, 700.0), (470.0, 640.0));
    assert!(matches!(
        sent.first(),
        Some(Command::Reparent {
            parent_id: None,
            pos: None,
            ..
        })
    ));
    let c3 = node(&a, "c3");
    assert_eq!((c3.parent_id, c3.x, c3.y), (None, 800.0, 700.0));
    assert_eq!(steps(&a), 1);
}

#[test]
fn a_flow_child_dropped_on_a_frame_that_is_not_in_its_own_parent_goes_into_it() {
    let mut a = app_of(cards(), true);
    // `other` is a free frame at (100, 400): the world box of `c3` (330, 60) plus the drag is (150, 420) there.
    let sent = drop_node(&mut a, "c3", (150.0, 420.0), (-180.0, 360.0));
    assert!(
        matches!(sent.first(), Some(Command::Reparent { parent_id: Some(p), pos: None, .. }) if p == "other")
    );
    let c3 = node(&a, "c3");
    assert_eq!(
        (c3.parent_id.as_deref(), c3.x, c3.y),
        (Some("other"), 50.0, 20.0)
    );
    assert_eq!(box_of(&a, "c3"), (50.0, 20.0, 100.0, 100.0));
}

#[test]
fn a_node_dropped_into_an_empty_auto_layout_frame_becomes_its_only_child() {
    let mut a = app_of(cards(), true);
    let sent = drop_node(&mut a, "c1", (550.0, 430.0), (0.0, 0.0));
    assert!(matches!(
        sent.as_slice(),
        [Command::Reparent { parent_id: Some(p), pos: Some(pos), .. }] if p == "empty" && pos == "a0"
    ));
    assert_eq!(kids(&a, Some("empty")), ["c1"]);
    assert_eq!(box_of(&a, "c1"), (16.0, 16.0, 100.0, 100.0));
}

#[test]
fn a_drop_over_a_frame_inside_the_dragged_node_never_nests_it_into_itself() {
    let mut a = app_of(cards(), true);
    // `y` is a layout frame two levels inside `x`, under the pointer, and `tree` is the frame `x` is in.
    let sent = drop_node(&mut a, "x", (150.0, 550.0), (30.0, 40.0));
    assert!(matches!(sent.as_slice(), [Command::UpdateNode { id, .. }] if id == "x"));
    assert!(refusals(&mut a).is_empty());
    let x = node(&a, "x");
    assert_eq!(
        (x.parent_id.as_deref(), x.x, x.y),
        (Some("tree"), 50.0, 60.0)
    );
}

#[test]
fn redo_brings_a_drop_back_after_an_undo() {
    let mut a = app_of(cards(), true);
    drop_node(&mut a, "c3", (270.0, 100.0), (0.0, 0.0));
    a.dispatch(Action::Undo {});
    assert_eq!(kids(&a, Some("deck")), ["c1", "c2", "c3"]);
    a.dispatch(Action::Redo {});
    assert_eq!(kids(&a, Some("deck")), ["c1", "c3", "c2"]);
    // A drop that reparents and sets the position in one step comes back whole too.
    drop_node(&mut a, "c1", (150.0, 420.0), (0.0, 0.0));
    let done = node(&a, "c1");
    a.dispatch(Action::Undo {});
    assert_eq!(node(&a, "c1").parent_id.as_deref(), Some("deck"));
    a.dispatch(Action::Redo {});
    let again = node(&a, "c1");
    assert_eq!(
        (again.parent_id.clone(), again.x, again.y),
        (done.parent_id.clone(), done.x, done.y)
    );
    assert_eq!(again.parent_id.as_deref(), Some("other"));
}

// --- the flag off, refusals ---------------------------------------------------------------------------------------

#[test]
fn with_the_flag_off_a_drop_is_the_plain_move_it_always_was() {
    let mut a = app(false);
    let sent = drop_node(&mut a, "b", (30.0, 15.0), (3.0, -20.0));
    assert!(matches!(sent.as_slice(), [Command::UpdateNode { id, .. }] if id == "b"));
    let b = node(&a, "b");
    assert_eq!((b.parent_id.as_deref(), b.x, b.y), (Some("f"), 3.0, -20.0));
    assert_eq!(kids(&a, Some("f")), ["a", "b", "c", "abs"]);
}

#[test]
fn a_drop_of_an_unknown_node_or_at_a_point_that_is_not_a_number_is_refused_and_changes_nothing() {
    let mut a = app(true);
    assert!(drop_node(&mut a, "nope", (1.0, 1.0), (0.0, 0.0)).is_empty());
    assert_eq!(refusals(&mut a), ["not_found"]);
    assert!(drop_node(&mut a, "b", (f64::NAN, 1.0), (0.0, 0.0)).is_empty());
    assert_eq!(refusals(&mut a), ["invalid"]);
    assert!(drop_node(&mut a, "b", (1.0, 1.0), (f64::NAN, 0.0)).is_empty());
    assert_eq!(refusals(&mut a), ["invalid"]);
    assert!(drop_node(&mut a, "b", (1.0, 1.0), (0.0, f64::INFINITY)).is_empty());
    assert_eq!(refusals(&mut a), ["invalid"]);
    assert_eq!(steps(&a), 0);
}

#[test]
fn the_drop_action_reads_from_json() {
    let a: Action = serde_json::from_value(
        json!({"type": "dropNode", "id": "b", "x": 1.0, "y": 2.0, "dx": 3.0, "dy": 4.0}),
    )
    .unwrap();
    assert!(matches!(a, Action::DropNode { dx, .. } if dx == 3.0));
    assert!(serde_json::from_value::<Action>(json!({"type": "dropNode", "id": "b"})).is_err());
}
