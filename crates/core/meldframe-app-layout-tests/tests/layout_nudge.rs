//! A nudge (`Action::Nudge`, E8.1.1): the selection moves by one step in world space, as an arrow key does. A node that is
//! not in a flow moves by 1 unit (10 when the step is coarse) along the one axis of the direction, a child of an auto
//! layout frame goes one place along its flow (a plain press only), and every press is one undo step. With the host flag
//! `layout_on_canvas` off the canvas draws the stored boxes and every node moves, a flow child included, as a drag does.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::pointer::halt;
use layout_fixture::*;
use meldframe_app::{Action, App, Direction, Effect, Event};
use meldframe_protocol::Command;

fn select(a: &mut App, ids: &[&str]) {
    a.dispatch(Action::Select {
        ids: ids.iter().map(|s| s.to_string()).collect(),
    });
    a.take_events();
}

fn nudge(a: &mut App, direction: Direction, coarse: bool) -> Vec<Command> {
    let fx = a.dispatch(Action::Nudge { direction, coarse });
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.command.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

fn xy(a: &App, id: &str) -> (f64, f64) {
    let n = node(a, id);
    (n.x, n.y)
}

fn undo(a: &mut App) {
    a.dispatch(Action::Undo {});
    a.take_events();
}

#[test]
fn a_free_node_moves_one_unit_in_each_direction_and_ten_when_coarse() {
    let mut a = app(true);
    select(&mut a, &["fg"]);
    assert_eq!(xy(&a, "fg"), (20.0, 20.0));
    for (d, coarse, to) in [
        (Direction::Right, false, (21.0, 20.0)),
        (Direction::Down, false, (21.0, 21.0)),
        (Direction::Left, false, (20.0, 21.0)),
        (Direction::Up, false, (20.0, 20.0)),
        (Direction::Right, true, (30.0, 20.0)),
        (Direction::Down, true, (30.0, 30.0)),
        (Direction::Left, true, (20.0, 30.0)),
        (Direction::Up, true, (20.0, 20.0)),
    ] {
        nudge(&mut a, d, coarse);
        assert_eq!(xy(&a, "fg"), to, "{d:?} coarse {coarse}");
    }
    assert_eq!(steps(&a), 8);
}

#[test]
fn a_nudge_writes_the_one_axis_it_moves_and_is_one_step_the_editor_can_undo() {
    let mut a = app(true);
    select(&mut a, &["fg", "g"]);
    let before = snapshot(a.nodes());
    // `g` holds `fg`: the parent moves and the child follows it, so only `g` is written.
    let sent = nudge(&mut a, Direction::Right, false);
    assert_eq!(sent.len(), 1);
    assert!(matches!(&sent[0], Command::UpdateNode { id, patch }
        if id == "g" && patch.x == Some(401.0) && patch.y.is_none()));
    assert_eq!((xy(&a, "g"), xy(&a, "fg")), ((401.0, 0.0), (20.0, 20.0)));
    assert!(a.take_events().iter().any(|e| matches!(
        e,
        Event::DocumentChanged {
            optimistic: true,
            ..
        }
    )));
    assert_eq!(steps(&a), 1);
    undo(&mut a);
    assert_eq!(snapshot(a.nodes()), before);
}

#[test]
fn several_nodes_move_together_as_one_step() {
    let mut a = app(true);
    select(&mut a, &["fg", "abs", "g"]);
    nudge(&mut a, Direction::Down, true);
    assert_eq!(xy(&a, "g"), (400.0, 10.0));
    assert_eq!(xy(&a, "abs"), (150.0, 210.0));
    assert_eq!(xy(&a, "fg"), (20.0, 20.0));
    assert_eq!(steps(&a), 1);
    undo(&mut a);
    assert_eq!((xy(&a, "g"), xy(&a, "abs")), ((400.0, 0.0), (150.0, 200.0)));
}

#[test]
fn a_selected_child_of_a_selected_node_follows_it_and_is_not_moved_twice() {
    let mut a = app_of(nested(), true);
    select(&mut a, &["q", "p"]);
    nudge(&mut a, Direction::Left, false);
    assert_eq!((xy(&a, "p"), xy(&a, "q")), ((9.0, 20.0), (5.0, 5.0)));
}

#[test]
fn a_child_of_a_flow_child_moves_though_its_parent_does_not() {
    let mut a = app_of(nested(), true);
    // `k` is in the flow of `col` and stays; `kk` is free inside it, so it moves even with `k` in the selection.
    select(&mut a, &["k", "kk"]);
    nudge(&mut a, Direction::Right, false);
    assert_eq!(xy(&a, "k"), (0.0, 0.0));
    assert_eq!(xy(&a, "kk"), (8.0, 7.0));
}

#[test]
fn a_flow_child_never_gets_a_stored_position_with_the_layout_on() {
    let mut a = app(true);
    select(&mut a, &["b"]);
    // Across the axis of the column: nothing, and the app says nothing happened.
    for (d, coarse) in [
        (Direction::Left, false),
        (Direction::Right, true),
        (Direction::Down, true),
        (Direction::Up, true),
    ] {
        assert!(nudge(&mut a, d, coarse).is_empty(), "{d:?} coarse {coarse}");
        assert_eq!(a.take_events(), vec![], "{d:?} coarse {coarse}");
    }
    assert_eq!(steps(&a), 0);
    // Along it, a plain press is a move of one place and writes no position.
    let sent = nudge(&mut a, Direction::Down, false);
    assert!(matches!(sent[..], [Command::Reorder { .. }]));
    assert_eq!(kids(&a, Some("f")), ["a", "c", "b", "abs"]);
    assert_eq!(xy(&a, "b"), (0.0, 0.0));
}

#[test]
fn a_mixed_selection_does_what_each_node_is_owed_in_one_step() {
    let mut a = app(true);
    select(&mut a, &["a", "fg", "abs"]);
    nudge(&mut a, Direction::Down, false);
    assert_eq!(kids(&a, Some("f")), ["b", "a", "c", "abs"]);
    assert_eq!(
        (xy(&a, "fg"), xy(&a, "abs")),
        ((20.0, 21.0), (150.0, 201.0))
    );
    assert_eq!(steps(&a), 1);
    // Across the axis the flow child stays, the others move.
    nudge(&mut a, Direction::Right, true);
    assert_eq!(kids(&a, Some("f")), ["b", "a", "c", "abs"]);
    assert_eq!(
        (xy(&a, "fg"), xy(&a, "abs")),
        ((30.0, 21.0), (160.0, 201.0))
    );
    assert_eq!(steps(&a), 2);
}

#[test]
fn with_the_layout_off_the_canvas_draws_stored_boxes_and_a_flow_child_moves_like_a_drag() {
    let mut a = app(false);
    select(&mut a, &["a"]);
    nudge(&mut a, Direction::Down, false);
    nudge(&mut a, Direction::Right, true);
    assert_eq!(xy(&a, "a"), (10.0, 1.0));
    assert_eq!(kids(&a, Some("f")), ["a", "b", "c", "abs"]);
    assert_eq!(steps(&a), 2);
}

#[test]
fn a_nudge_is_nothing_with_no_selection_when_halted_and_refused_past_the_limit_of_a_coordinate() {
    let mut a = app(true);
    assert!(nudge(&mut a, Direction::Right, false).is_empty());
    assert_eq!(a.take_events(), vec![]);
    a.dispatch(Action::Move {
        id: "g".into(),
        x: 1e6,
        y: 0.0,
    });
    a.take_events();
    select(&mut a, &["g"]);
    let steps_before = steps(&a);
    // One unit past the limit: the press is refused whole and nothing changes.
    assert!(nudge(&mut a, Direction::Right, false).is_empty());
    assert_eq!(refusals(&mut a), ["invalid"]);
    assert_eq!((xy(&a, "g"), steps(&a)), ((1e6, 0.0), steps_before));
    // The other way along the same axis is fine.
    nudge(&mut a, Direction::Left, false);
    assert_eq!(xy(&a, "g"), (1e6 - 1.0, 0.0));
    halt(&mut a);
    a.take_events();
    assert!(nudge(&mut a, Direction::Down, false).is_empty());
    assert_eq!(xy(&a, "g"), (1e6 - 1.0, 0.0));
}

#[test]
fn a_selected_frame_and_a_flow_child_of_it_act_once_the_child_follows_the_frame() {
    let mut a = app_of(nested(), true);
    select(&mut a, &["col", "k"]);
    nudge(&mut a, Direction::Down, false);
    assert_eq!(
        kids(&a, Some("col")),
        ["k", "k2"],
        "the child follows, it is not put in another place"
    );
    assert_eq!(xy(&a, "col"), (300.0, 1.0));
    assert_eq!(steps(&a), 1);
    // The child alone does go one place along the flow.
    select(&mut a, &["k"]);
    nudge(&mut a, Direction::Down, false);
    assert_eq!(kids(&a, Some("col")), ["k2", "k"]);
}

#[test]
fn can_nudge_asks_about_the_press_and_agrees_with_it() {
    let mut a = app(true);
    assert!(!a.can_nudge(Direction::Right, false));
    select(&mut a, &["fg"]);
    assert!(a.can_nudge(Direction::Right, true));
    // A flow child goes along its flow and stays under a coarse press or the key across the axis.
    select(&mut a, &["a"]);
    assert!(a.can_nudge(Direction::Down, false));
    assert!(!a.can_nudge(Direction::Down, true));
    assert!(!a.can_nudge(Direction::Right, false));
    assert!(!a.can_nudge(Direction::Right, true));
    // Both children of the row: neither has a free neighbour to swap with.
    select(&mut a, &["r1", "r2"]);
    assert!(!a.can_nudge(Direction::Right, false));
    select(&mut a, &["r1", "fg"]);
    assert!(a.can_nudge(Direction::Right, false));
    // With the layout off the stored boxes are drawn and a flow child moves.
    let mut off = app(false);
    select(&mut off, &["a"]);
    assert!(off.can_nudge(Direction::Right, true));
}

#[test]
fn can_nudge_is_false_for_a_press_the_app_would_refuse() {
    let mut a = app(true);
    for (x, y) in [(1e6, 0.0), (1e6, 1e6)] {
        a.dispatch(Action::Move {
            id: "g".into(),
            x,
            y,
        });
    }
    a.take_events();
    select(&mut a, &["g"]);
    assert!(!a.can_nudge(Direction::Right, true), "past the limit of x");
    assert!(!a.can_nudge(Direction::Down, false), "past the limit of y");
    assert!(a.can_nudge(Direction::Left, false));
    assert!(a.can_nudge(Direction::Up, true));
    // What it says is what the press does.
    let before = snapshot(a.nodes());
    assert!(nudge(&mut a, Direction::Right, true).is_empty());
    assert_eq!(refusals(&mut a), ["invalid"]);
    assert_eq!(snapshot(a.nodes()), before);
}
