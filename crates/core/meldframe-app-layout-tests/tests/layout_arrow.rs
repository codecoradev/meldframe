//! An arrow key moves the selected children of an auto layout frame one place along its flow (`Action::ArrowKey`), as one
//! undo step that writes no `x` or `y`; the key across the axis, a node outside a flow and a selection that spans two
//! frames do nothing; with the host flag off the key does nothing at all.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::{Action, App, Direction, Effect};
use meldframe_protocol::Command;

const KEYS: [Direction; 4] = [
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
];

fn select(a: &mut App, ids: &[&str]) {
    a.dispatch(Action::Select {
        ids: ids.iter().map(|s| s.to_string()).collect(),
    });
    a.take_events();
}

/// Presses the key; returns the commands that were sent.
fn press(a: &mut App, key: Direction) -> Vec<Command> {
    let fx = a.dispatch(Action::ArrowKey { direction: key });
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.command.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

/// The children of `f` that the layout places, in order (`abs` is out of the flow).
fn flow_f(a: &App) -> Vec<String> {
    let mut k = kids(a, Some("f"));
    k.retain(|id| id != "abs");
    k
}

fn order(a: &App) -> Vec<&'static str> {
    // The names of `flow_f` as text, for a short comparison.
    let all = ["a", "b", "c"];
    flow_f(a)
        .iter()
        .map(|id| *all.iter().find(|x| **x == id).expect("a, b or c"))
        .collect()
}

fn stored_xy(a: &App, id: &str) -> (f64, f64) {
    let n = node(a, id);
    (n.x, n.y)
}

#[test]
fn up_and_down_move_a_child_of_a_column_one_place_in_one_step() {
    let mut a = app(true);
    select(&mut a, &["b"]);
    let sent = press(&mut a, Direction::Up);
    assert_eq!(order(&a), ["b", "a", "c"]);
    assert!(
        !sent.is_empty() && sent.iter().all(|c| matches!(c, Command::Reorder { .. })),
        "{sent:?}"
    );
    assert_eq!(steps(&a), 1);
    press(&mut a, Direction::Down);
    press(&mut a, Direction::Down);
    assert_eq!(order(&a), ["a", "c", "b"]);
    assert_eq!(steps(&a), 3, "a key, a step");
    a.dispatch(Action::Undo {});
    assert_eq!(order(&a), ["a", "b", "c"], "one undo takes back one key");
}

#[test]
fn a_key_writes_no_stored_position() {
    let mut a = app(true);
    let before = (stored_xy(&a, "a"), stored_xy(&a, "b"), stored_xy(&a, "c"));
    select(&mut a, &["b"]);
    press(&mut a, Direction::Up);
    press(&mut a, Direction::Down);
    assert_eq!(
        (stored_xy(&a, "a"), stored_xy(&a, "b"), stored_xy(&a, "c")),
        before
    );
}

#[test]
fn left_and_right_move_a_child_of_a_row_and_up_and_down_do_nothing_there() {
    let mut a = app(true);
    select(&mut a, &["r1"]);
    assert!(press(&mut a, Direction::Up).is_empty());
    assert!(press(&mut a, Direction::Down).is_empty());
    assert_eq!(steps(&a), 0);
    press(&mut a, Direction::Right);
    assert_eq!(kids(&a, Some("r")), ["r2", "r1"]);
    press(&mut a, Direction::Left);
    assert_eq!(kids(&a, Some("r")), ["r1", "r2"]);
    assert_eq!(steps(&a), 2);
}

#[test]
fn the_key_across_the_axis_does_nothing_and_neither_does_the_end_of_the_flow() {
    let mut a = app(true);
    select(&mut a, &["b"]);
    for k in [Direction::Left, Direction::Right] {
        assert!(press(&mut a, k).is_empty(), "{k:?} across a column");
    }
    select(&mut a, &["a"]);
    assert!(
        press(&mut a, Direction::Up).is_empty(),
        "the first stays first"
    );
    select(&mut a, &["c"]);
    assert!(
        press(&mut a, Direction::Down).is_empty(),
        "the last stays last, the absolute child is not a place"
    );
    assert_eq!(steps(&a), 0);
    assert_eq!(order(&a), ["a", "b", "c"]);
    assert!(refusals(&mut a).is_empty(), "nothing is refused either");
}

#[test]
fn a_node_outside_a_flow_is_left_alone() {
    let mut a = app(true);
    for id in ["abs", "fg", "g", "f"] {
        select(&mut a, &[id]);
        for k in KEYS {
            assert!(press(&mut a, k).is_empty(), "{id} {k:?}");
        }
    }
    assert_eq!(steps(&a), 0);
}

#[test]
fn a_selection_of_several_children_moves_together_and_stops_at_the_end() {
    let mut a = app(true);
    select(&mut a, &["a", "b"]);
    press(&mut a, Direction::Down);
    assert_eq!(order(&a), ["c", "a", "b"]);
    assert_eq!(steps(&a), 1, "one step for the group");
    a.dispatch(Action::Undo {});
    // Not next to each other: each goes one place where it can; the last one is at the end already.
    select(&mut a, &["a", "c"]);
    press(&mut a, Direction::Down);
    assert_eq!(order(&a), ["b", "a", "c"]);
    a.dispatch(Action::Undo {});
    select(&mut a, &["b", "c"]);
    press(&mut a, Direction::Up);
    assert_eq!(order(&a), ["b", "c", "a"]);
    press(&mut a, Direction::Up);
    assert_eq!(
        order(&a),
        ["b", "c", "a"],
        "the group is at the start: nothing moves"
    );
}

#[test]
fn flow_children_of_two_frames_in_the_selection_move_nothing() {
    let mut a = app(true);
    select(&mut a, &["b", "r1"]);
    for k in KEYS {
        assert!(press(&mut a, k).is_empty());
    }
    assert_eq!(steps(&a), 0);
}

#[test]
fn other_nodes_in_the_selection_are_left_alone() {
    let mut a = app(true);
    select(&mut a, &["b", "fg", "abs"]);
    press(&mut a, Direction::Up);
    assert_eq!(order(&a), ["b", "a", "c"]);
    assert_eq!(stored_xy(&a, "fg"), (20.0, 20.0));
    assert_eq!(stored_xy(&a, "abs"), (150.0, 200.0));
}

#[test]
fn with_the_flag_off_the_key_does_nothing_at_all() {
    let mut a = app(false);
    select(&mut a, &["b"]);
    let state = serde_json::to_string(a.state()).unwrap();
    let doc = serde_json::to_string(a.nodes()).unwrap();
    for k in KEYS {
        let fx = a.dispatch(Action::ArrowKey { direction: k });
        assert!(fx.is_empty(), "{k:?}: no effect");
        assert!(a.take_events().is_empty(), "{k:?}: no event");
    }
    assert_eq!(serde_json::to_string(a.state()).unwrap(), state);
    assert_eq!(serde_json::to_string(a.nodes()).unwrap(), doc);
}

#[test]
fn the_action_reads_from_json() {
    let a: Action = serde_json::from_str(r#"{"type":"arrowKey","direction":"up"}"#).unwrap();
    assert!(matches!(
        a,
        Action::ArrowKey {
            direction: Direction::Up
        }
    ));
    assert!(serde_json::from_str::<Action>(r#"{"type":"arrowKey","direction":"north"}"#).is_err());
}

#[test]
fn the_reorder_action_of_a_layers_panel_writes_the_order_and_no_position() {
    // The Layers panel of the web has no drag-reorder yet; when it gets one it sends `Action::Reorder`, which must be the
    // one rule of the order: a `Reorder` command and nothing else, one step, the stored x and y of a flow child as they were.
    for on in [true, false] {
        let mut a = app(on);
        let before = stored_xy(&a, "c");
        let fx = a.dispatch(Action::Reorder {
            id: "c".into(),
            before: Some("a".into()),
            after: None,
        });
        let sent: Vec<Command> = fx
            .iter()
            .flat_map(|e| match e {
                Effect::SendOps { ops } => ops.iter().map(|o| o.command.clone()).collect(),
                _ => vec![],
            })
            .collect();
        assert!(
            matches!(sent.as_slice(), [Command::Reorder { .. }]),
            "{on}: {sent:?}"
        );
        assert_eq!(flow_f(&a), ["c", "a", "b"], "{on}");
        assert_eq!((stored_xy(&a, "c"), steps(&a)), (before, 1), "{on}");
    }
}
