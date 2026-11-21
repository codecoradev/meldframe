//! A drop of several selected nodes on a laid-out canvas (`Action::DropNodes`): the flow children of one frame move as a
//! block to the slot at the pointer, keeping their order; the others move by the drag; flow children of several frames stay.
//! No stored position of a flow child is written. With the host flag off it is the move a drag always was.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::{Action, App};
use meldframe_protocol::{Command, SizeMode};
use serde_json::json;

fn only_reorders(sent: &[Command]) -> bool {
    !sent.is_empty() && sent.iter().all(|c| matches!(c, Command::Reorder { .. }))
}

fn stored(a: &App, id: &str) -> (f64, f64) {
    let n = node(a, id);
    (n.x, n.y)
}

#[test]
fn flow_children_of_one_frame_move_as_a_block_to_the_slot_keeping_their_order() {
    let mut a = app(true);
    let xy = (stored(&a, "a"), stored(&a, "b"));
    // `c` is the only other flow child (70 to 110, middle 90): the pointer is past it, so the block goes last.
    let sent = drop_nodes(&mut a, &["b", "a"], (30.0, 100.0), (0.0, 80.0));
    assert!(only_reorders(&sent), "{sent:?}");
    assert_eq!(
        kids(&a, Some("f"))[..3],
        ["c", "a", "b"],
        "a before b, as in the flow, whatever the order of the ids"
    );
    assert_eq!(
        (stored(&a, "a"), stored(&a, "b")),
        xy,
        "no stored position of a flow child is written"
    );
    assert_eq!(steps(&a), 1);
    a.dispatch(Action::Undo {});
    assert_eq!(kids(&a, Some("f"))[..3], ["a", "b", "c"]);
}

#[test]
fn the_block_goes_to_the_start_and_gathers_children_that_were_apart() {
    let mut a = app(true);
    drop_nodes(&mut a, &["b", "c"], (30.0, 12.0), (0.0, -50.0));
    assert_eq!(kids(&a, Some("f"))[..3], ["b", "c", "a"]);
    let mut a = app(true);
    // `a` and `c` with `b` between them, the pointer before `b`'s middle: they meet in front of `b`.
    drop_nodes(&mut a, &["a", "c"], (30.0, 40.0), (0.0, 5.0));
    assert_eq!(kids(&a, Some("f"))[..3], ["a", "c", "b"]);
}

#[test]
fn a_block_that_is_in_one_piece_at_the_slot_already_makes_no_step() {
    let mut a = app(true);
    let sent = drop_nodes(&mut a, &["a", "b"], (30.0, 40.0), (0.0, 5.0));
    assert!(sent.is_empty(), "{sent:?}");
    assert_eq!(steps(&a), 0);
    assert!(refusals(&mut a).is_empty());
}

#[test]
fn the_others_move_by_the_drag_and_the_flow_children_go_to_the_slot() {
    let mut a = app(true);
    drop_nodes(&mut a, &["a", "fg"], (30.0, 100.0), (3.0, 4.0));
    assert_eq!(kids(&a, Some("f"))[..3], ["b", "c", "a"]);
    assert_eq!(stored(&a, "fg"), (23.0, 24.0));
    assert_eq!(
        stored(&a, "a"),
        (0.0, 0.0),
        "the flow child keeps the position it had stored"
    );
    assert_eq!(steps(&a), 1);
}

#[test]
fn flow_children_of_two_frames_stay_and_the_others_move() {
    let mut a = app(true);
    let sent = drop_nodes(&mut a, &["b", "r1", "fg"], (30.0, 100.0), (3.0, 4.0));
    assert!(
        matches!(sent.as_slice(), [Command::UpdateNode { id, .. }] if id == "fg"),
        "{sent:?}"
    );
    assert_eq!(kids(&a, Some("f"))[..3], ["a", "b", "c"]);
    assert_eq!(kids(&a, Some("r")), ["r1", "r2"]);
    assert_eq!(stored(&a, "fg"), (23.0, 24.0));
    // Only flow children of two frames: nothing at all.
    let mut a = app(true);
    assert!(drop_nodes(&mut a, &["b", "r1"], (30.0, 100.0), (3.0, 4.0)).is_empty());
    assert_eq!(steps(&a), 0);
}

#[test]
fn a_block_over_another_auto_layout_frame_goes_into_its_flow_in_order() {
    let mut a = app(true);
    let xy = (stored(&a, "a"), stored(&a, "b"));
    // `r` is a row; `r1` is 716 to 756 and `r2` 766 to 806: the pointer at 760 is past the middle of `r1` only.
    let sent = drop_nodes(&mut a, &["b", "a"], (760.0, 50.0), (730.0, 0.0));
    assert!(
        sent.iter().all(|c| matches!(c, Command::Reparent { .. })),
        "{sent:?}"
    );
    assert_eq!(kids(&a, Some("r")), ["r1", "a", "b", "r2"]);
    assert_eq!((stored(&a, "a"), stored(&a, "b")), xy);
    assert_eq!(steps(&a), 1);
    a.dispatch(Action::Undo {});
    assert_eq!(kids(&a, Some("r")), ["r1", "r2"]);
    assert_eq!(kids(&a, Some("f"))[..3], ["a", "b", "c"]);
}

#[test]
fn a_block_over_a_free_frame_leaves_the_flow_each_at_the_place_it_was_drawn_plus_the_drag() {
    let mut a = app_of(with_sized_b("fill"), true);
    // `a` is drawn at (10, 10), `b` at (10, 35) and 180 wide; `g` is the free frame at (400, 0).
    drop_nodes(&mut a, &["a", "b"], (450.0, 50.0), (440.0, 30.0));
    for id in ["a", "b"] {
        assert_eq!(node(&a, id).parent_id.as_deref(), Some("g"), "{id}");
    }
    assert_eq!(stored(&a, "a"), (50.0, 40.0));
    assert_eq!(stored(&a, "b"), (50.0, 65.0));
    let b = node(&a, "b");
    assert_eq!(
        (b.sw, b.w),
        (SizeMode::Fixed, 180.0),
        "a fill means nothing outside a flow"
    );
    assert_eq!(steps(&a), 1);
    a.dispatch(Action::Undo {});
    assert_eq!(node(&a, "a").parent_id.as_deref(), Some("f"));
}

#[test]
fn a_block_off_every_frame_goes_to_the_page() {
    let mut a = app(true);
    drop_nodes(&mut a, &["a", "b"], (50.0, 900.0), (0.0, 890.0));
    for id in ["a", "b"] {
        assert_eq!(node(&a, id).parent_id, None, "{id}");
    }
    assert_eq!(stored(&a, "a"), (10.0, 900.0));
}

#[test]
fn a_node_and_its_selected_descendant_are_one_node() {
    let mut a = app(true);
    // `f` is a free frame at the top and `a` is inside it: only `f` moves, `a` follows as its child.
    let sent = drop_nodes(&mut a, &["f", "a"], (500.0, 500.0), (20.0, 30.0));
    assert!(
        matches!(sent.as_slice(), [Command::UpdateNode { id, .. }] if id == "f"),
        "{sent:?}"
    );
    assert_eq!(stored(&a, "f"), (20.0, 30.0));
    assert_eq!(stored(&a, "a"), (0.0, 0.0));
}

#[test]
fn one_node_is_what_drop_node_does_to_the_bit() {
    for (id, at) in [
        ("a", (30.0, 100.0)),
        ("b", (720.0, 50.0)),
        ("a", (450.0, 50.0)),
        ("fg", (30.0, 60.0)),
        ("g", (5.0, 5.0)),
    ] {
        for on in [true, false] {
            let (mut one, mut many) = (app(on), app(on));
            let a = serde_json::to_string(&drop_node(&mut one, id, at, (7.0, 9.0))).unwrap();
            let b = serde_json::to_string(&drop_nodes(&mut many, &[id], at, (7.0, 9.0))).unwrap();
            assert_eq!(a, b, "{id} {at:?} {on}");
            assert_eq!(
                serde_json::to_string(one.nodes()).unwrap(),
                serde_json::to_string(many.nodes()).unwrap()
            );
            assert_eq!(steps(&one), steps(&many));
        }
    }
}

#[test]
fn a_number_that_is_not_one_is_refused_and_changes_nothing() {
    for on in [true, false] {
        let mut a = app(on);
        let doc = serde_json::to_string(a.nodes()).unwrap();
        assert!(drop_nodes(&mut a, &["a", "b"], (1.0, 1.0), (f64::NAN, 1.0)).is_empty());
        assert!(!refusals(&mut a).is_empty(), "{on}");
        assert_eq!(serde_json::to_string(a.nodes()).unwrap(), doc);
        assert_eq!(steps(&a), 0);
    }
}

#[test]
fn an_id_that_is_gone_is_skipped_and_the_others_are_dropped() {
    // Someone deleted a node while the drag went on: the ids the page sends still name it.
    for on in [true, false] {
        let mut a = app(on);
        let sent = drop_nodes(&mut a, &["a", "nope", "fg"], (30.0, 100.0), (3.0, 4.0));
        assert!(!sent.is_empty(), "{on}: the survivors are dropped");
        assert!(refusals(&mut a).is_empty(), "{on}: nothing is refused");
        assert_eq!(steps(&a), if on { 1 } else { 2 }, "{on}");
        if on {
            // `a` goes to the slot, `fg` moves by the drag.
            assert_eq!(kids(&a, Some("f"))[..3], ["b", "c", "a"]);
            assert_eq!(stored(&a, "fg"), (23.0, 24.0));
        } else {
            // The move it always was: both by the drag, each on a step of its own (the ids are not the selection).
            assert_eq!(stored(&a, "a"), (3.0, 4.0));
            assert_eq!(stored(&a, "fg"), (23.0, 24.0));
        }
    }
    // One survivor is a drop of one node.
    for on in [true, false] {
        let (mut one, mut many) = (app(on), app(on));
        let a =
            serde_json::to_string(&drop_node(&mut one, "a", (30.0, 100.0), (3.0, 4.0))).unwrap();
        let b = serde_json::to_string(&drop_nodes(
            &mut many,
            &["nope", "a"],
            (30.0, 100.0),
            (3.0, 4.0),
        ))
        .unwrap();
        assert_eq!(a, b, "{on}");
    }
}

#[test]
fn when_no_id_is_a_node_the_drop_is_refused_as_not_found_and_nothing_is_applied() {
    for on in [true, false] {
        let mut a = app(on);
        let doc = serde_json::to_string(a.nodes()).unwrap();
        assert!(drop_nodes(&mut a, &["nope", "nor-this"], (1.0, 1.0), (1.0, 1.0)).is_empty());
        assert_eq!(refusals(&mut a), ["not_found"], "{on}");
        assert_eq!(serde_json::to_string(a.nodes()).unwrap(), doc);
        assert_eq!(steps(&a), 0);
        // And the preview has nothing to say.
        let ids = ["nope".to_string()];
        assert!(a.drop_preview_many(&ids, (1.0, 1.0), (1.0, 1.0)).is_none());
    }
}

// --- the flag off: the move a drag always was ---------------------------------------------------------------------

/// What the web sent for a drag before this action: the selection as one `moveBy` when it is the dragged ids and none is
/// under another, else a `move` of each top node.
fn legacy(a: &mut App, ids: &[&str], tops: &[&str], by: (f64, f64)) -> String {
    let selection = a.state().selection.clone();
    let whole = tops.len() == ids.len()
        && ids.len() == selection.len()
        && ids.iter().all(|i| selection.iter().any(|s| s == i));
    let mut sent = Vec::new();
    if whole {
        sent.push(a.dispatch(Action::MoveBy { dx: by.0, dy: by.1 }));
    } else {
        for id in tops {
            let n = node(a, id);
            sent.push(a.dispatch(Action::Move {
                id: id.to_string(),
                x: n.x + by.0,
                y: n.y + by.1,
            }));
        }
    }
    serde_json::to_string(&sent).unwrap()
}

fn select(a: &mut App, ids: &[&str]) {
    a.dispatch(Action::Select {
        ids: ids.iter().map(|s| s.to_string()).collect(),
    });
    a.take_events();
}

#[test]
fn with_the_flag_off_a_drop_of_the_selection_is_the_move_it_was() {
    // (dragged ids, the selection, the top nodes among the ids)
    let cases: [(&[&str], &[&str], &[&str]); 4] = [
        (&["a", "b"], &["a", "b"], &["a", "b"]),
        (&["a", "fg", "r1"], &["a", "fg", "r1"], &["a", "fg", "r1"]),
        // A node and its descendant among three: the tops are moved one by one, each on its own step.
        (&["f", "a", "g"], &["f", "a", "g"], &["f", "g"]),
        // Dragged ids that are not the selection.
        (&["a", "b"], &["a", "c"], &["a", "b"]),
    ];
    for (ids, selection, tops) in cases {
        let (mut now, mut then) = (app(false), app(false));
        select(&mut now, selection);
        select(&mut then, selection);
        let effects = now.dispatch(Action::DropNodes {
            ids: ids.iter().map(|s| s.to_string()).collect(),
            x: 123.0,
            y: 456.0,
            dx: 11.0,
            dy: -6.0,
        });
        let old = legacy(&mut then, ids, tops, (11.0, -6.0));
        // The ops sent, in order (a group of moves is one effect now and was one effect each before: the ops are the same).
        let ops = |fx: &[meldframe_app::Effect]| -> String {
            let v: Vec<_> = fx
                .iter()
                .flat_map(|e| match e {
                    meldframe_app::Effect::SendOps { ops } => ops
                        .iter()
                        .map(|o| serde_json::to_value(&o.command).unwrap())
                        .collect(),
                    _ => vec![],
                })
                .collect();
            serde_json::to_string(&v).unwrap()
        };
        let before: serde_json::Value = serde_json::from_str(&old).unwrap();
        let old_ops: Vec<serde_json::Value> = before
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|fx| fx.as_array().unwrap().iter())
            .filter(|e| e["type"] == "sendOps")
            .flat_map(|e| {
                e["ops"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|o| o["command"].clone())
            })
            .collect();
        assert_eq!(
            ops(&effects),
            serde_json::to_string(&old_ops).unwrap(),
            "{ids:?} {selection:?}"
        );
        assert_eq!(
            serde_json::to_string(now.nodes()).unwrap(),
            serde_json::to_string(then.nodes()).unwrap()
        );
        assert_eq!(
            steps(&now),
            steps(&then),
            "{ids:?} {selection:?}: the same number of undo steps"
        );
        assert_eq!(json!(now.state().history), json!(then.state().history));
    }
}
