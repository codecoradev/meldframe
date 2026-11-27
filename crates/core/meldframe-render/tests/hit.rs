//! T9: what is under a point. One definition (`HitIndex`), asked of the boxes the canvas draws. `tests/fixtures/hit.json`
//! holds scenes and probes (a point and the id that is front-most there, or none); the app layer and the wasm bridge run
//! the same file (`crates/app/meldframe-wasm/tests/hit.rs`), so the rule is the same through the bridge.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::{Rect, StubMeasurer};
use meldframe_protocol::{Node, Variables};
use meldframe_render::{
    HitBox, HitIndex, ModeChoice, contains_point, hit_test, resolve, resolve_laid_out,
};
use serde_json::Value;

const FIXTURES: &str = include_str!("fixtures/hit.json");

fn cases() -> Vec<Value> {
    let cases: Vec<Value> = serde_json::from_str(FIXTURES).unwrap();
    assert!(cases.len() >= 12);
    cases
}

fn nodes_of(case: &Value) -> Vec<Node> {
    serde_json::from_value(case["nodes"].clone()).unwrap()
}

fn probes(case: &Value) -> Vec<((f64, f64), Option<String>)> {
    case["probes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let at = p["at"].as_array().unwrap();
            (
                (at[0].as_f64().unwrap(), at[1].as_f64().unwrap()),
                p["hit"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

#[test]
fn every_probe_of_every_fixture_hits_what_the_fixture_says() {
    for case in cases() {
        let name = case["name"].as_str().unwrap();
        let nodes = nodes_of(&case);
        let boxes = if case["layout"].as_bool().unwrap_or(false) {
            resolve_laid_out(
                &nodes,
                &Variables::default(),
                &ModeChoice::default(),
                &StubMeasurer,
            )
        } else {
            resolve(&nodes)
        };
        let index = HitIndex::from_boxes(&boxes);
        for ((x, y), want) in probes(&case) {
            assert_eq!(
                index.front_most(x, y).map(str::to_owned),
                want,
                "{name} at ({x}, {y})"
            );
            assert_eq!(
                hit_test(&boxes, x, y),
                want,
                "{name} at ({x}, {y}), hit_test"
            );
        }
    }
}

#[test]
fn a_file_with_no_auto_layout_is_hit_the_same_laid_out_or_not() {
    for case in cases() {
        if case["layout"].as_bool().unwrap_or(false) {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        let nodes = nodes_of(&case);
        let stored = HitIndex::from_boxes(&resolve(&nodes));
        let laid = HitIndex::from_boxes(&resolve_laid_out(
            &nodes,
            &Variables::default(),
            &ModeChoice::default(),
            &StubMeasurer,
        ));
        for ((x, y), want) in probes(&case) {
            assert_eq!(laid.front_most(x, y).map(str::to_owned), want, "{name}");
            assert_eq!(stored.front_most(x, y), laid.front_most(x, y), "{name}");
        }
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

fn hb<'a>(id: &'a str, parent: Option<&'a str>, r: Rect, clip: bool, visible: bool) -> HitBox<'a> {
    HitBox {
        id,
        parent,
        rect: r,
        clip,
        visible,
    }
}

fn under<'a>(index: &'a HitIndex, x: f64, y: f64) -> Vec<&'a str> {
    index.under(x, y).collect()
}

#[test]
fn under_lists_every_box_at_the_point_front_most_first() {
    let index = HitIndex::new([
        hb("back", None, rect(0.0, 0.0, 100.0, 100.0), false, true),
        hb(
            "kid",
            Some("back"),
            rect(10.0, 10.0, 50.0, 50.0),
            false,
            true,
        ),
        hb("front", None, rect(20.0, 20.0, 100.0, 100.0), false, true),
    ]);
    assert_eq!(under(&index, 30.0, 30.0), ["front", "kid", "back"]);
    assert_eq!(under(&index, 5.0, 5.0), ["back"]);
    assert!(under(&index, 500.0, 5.0).is_empty());
    assert_eq!(index.front_most(30.0, 30.0), Some("front"));
}

#[test]
fn a_hidden_box_and_its_whole_subtree_are_not_under_a_point() {
    // `PaintedBox` has no visibility yet (ADR-0025 is proposed), so the rule is pinned on the neutral input: once the box
    // carries the field, one line hands it over and the rule is already here.
    let index = HitIndex::new([
        hb("shown", None, rect(0.0, 0.0, 100.0, 100.0), false, true),
        hb(
            "hidden",
            Some("shown"),
            rect(0.0, 0.0, 80.0, 80.0),
            false,
            false,
        ),
        hb(
            "under_hidden",
            Some("hidden"),
            rect(0.0, 0.0, 40.0, 40.0),
            false,
            true,
        ),
        hb(
            "sibling",
            Some("shown"),
            rect(50.0, 50.0, 10.0, 10.0),
            false,
            true,
        ),
    ]);
    assert_eq!(under(&index, 20.0, 20.0), ["shown"]);
    assert_eq!(under(&index, 55.0, 55.0), ["sibling", "shown"]);
}

#[test]
fn a_point_that_is_not_a_number_is_over_nothing() {
    let index = HitIndex::new([hb("a", None, rect(0.0, 0.0, 10.0, 10.0), false, true)]);
    for (x, y) in [
        (f64::NAN, 1.0),
        (1.0, f64::NAN),
        (f64::INFINITY, 1.0),
        (1.0, f64::NEG_INFINITY),
    ] {
        assert_eq!(index.front_most(x, y), None, "({x}, {y})");
    }
}

#[test]
fn the_first_of_two_boxes_with_one_id_is_the_one_that_counts() {
    let index = HitIndex::new([
        hb("a", None, rect(0.0, 0.0, 10.0, 10.0), false, true),
        hb("a", None, rect(50.0, 50.0, 10.0, 10.0), false, true),
    ]);
    assert_eq!(index.front_most(5.0, 5.0), Some("a"));
    assert_eq!(index.front_most(55.0, 55.0), None);
}

#[test]
fn a_child_listed_before_its_parent_has_no_parent_to_inherit_from() {
    let index = HitIndex::new([
        hb("kid", Some("late"), rect(0.0, 0.0, 10.0, 10.0), false, true),
        hb("late", None, rect(0.0, 0.0, 5.0, 5.0), true, true),
    ]);
    assert_eq!(under(&index, 8.0, 8.0), ["kid"]);
}

#[test]
fn contains_point_takes_the_left_and_top_edges_and_not_the_right_and_bottom_ones() {
    let r = rect(10.0, 20.0, 30.0, 40.0);
    assert!(contains_point(r, 10.0, 20.0));
    assert!(contains_point(r, 39.999, 59.999));
    assert!(!contains_point(r, 40.0, 30.0));
    assert!(!contains_point(r, 20.0, 60.0));
    assert!(!contains_point(r, 9.999, 30.0));
    assert!(!contains_point(r, f64::NAN, 30.0));
}

#[test]
fn a_box_of_the_scene_is_placed_by_the_offsets_of_its_ancestors() {
    let nodes: Vec<Node> = serde_json::from_value(serde_json::json!([
        {"id": "a", "type": "frame", "pos": "a0", "x": 100, "y": 100, "w": 400, "h": 400, "clip": false},
        {"id": "b", "type": "frame", "parentId": "a", "pos": "a0", "x": 50, "y": 50, "w": 200, "h": 200, "clip": false},
        {"id": "c", "type": "rect", "parentId": "b", "pos": "a0", "x": 5, "y": 5, "w": 10, "h": 10},
    ]))
    .unwrap();
    let boxes = resolve(&nodes);
    assert_eq!(hit_test(&boxes, 155.0, 155.0).as_deref(), Some("c"));
    assert_eq!(hit_test(&boxes, 156.0, 156.0).as_deref(), Some("c"));
    assert_eq!(hit_test(&boxes, 165.0, 165.0).as_deref(), Some("b"));
    assert_eq!(hit_test(&boxes, 5.0, 5.0), None);
}
