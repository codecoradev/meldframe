//! T9: `App::hit`, the query for what is under a world point. It answers from the scene the canvas draws, by the one rule of
//! `meldframe_render::HitIndex`, reads no state of the editor and changes none, and keeps the index of the scene for a
//! revision of the document, the measurer and the flag, so a hover that asks on every frame builds nothing.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, TextMeasurer, Tool};
use meldframe_protocol::Node;
use serde_json::{Value, json};

const FIXTURES: &str = include_str!("../../meldframe-render/tests/fixtures/hit.json");

fn nodes(list: Value) -> Vec<Node> {
    serde_json::from_value(list).unwrap()
}

fn app_of(list: Value, layout: bool) -> App {
    let mut a = App::new("user:me", "c0", 1, nodes(list), 0).unwrap();
    a.set_layout_on_canvas(layout);
    a.dispatch(Action::Connected { now_ms: 0 });
    a.take_events();
    a
}

/// Every one of the shared fixtures (the render crate holds the same file), through the app: the same probes give the same
/// ids, with the stored boxes and, for a file with no auto layout, with the laid-out ones too.
#[test]
fn the_app_answers_every_probe_of_the_shared_fixtures() {
    let cases: Vec<Value> = serde_json::from_str(FIXTURES).unwrap();
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        // A file whose parent is missing cannot be opened by the app (the check refuses it), so only the render crate reads it.
        if case["renderOnly"].as_bool().unwrap_or(false) {
            continue;
        }
        let layout_case = case["layout"].as_bool().unwrap_or(false);
        let flags: &[bool] = if layout_case { &[true] } else { &[false, true] };
        for &layout in flags {
            let a = app_of(case["nodes"].clone(), layout);
            for p in case["probes"].as_array().unwrap() {
                let (x, y) = (p["at"][0].as_f64().unwrap(), p["at"][1].as_f64().unwrap());
                assert_eq!(
                    a.hit(x, y).as_deref(),
                    p["hit"].as_str(),
                    "{name}, layout {layout}, at ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn an_empty_file_has_nothing_under_a_point() {
    assert_eq!(app_of(json!([]), true).hit(0.0, 0.0), None);
}

#[test]
fn hit_reads_no_state_and_changes_none() {
    let mut a = app_of(
        json!([{"id": "r", "type": "rect", "pos": "a0", "x": 0, "y": 0, "w": 50, "h": 50}]),
        true,
    );
    a.dispatch(Action::SetTool {
        tool: Tool::Rectangle,
    });
    a.take_events();
    let before = serde_json::to_string(a.state()).unwrap();
    let nodes_before = serde_json::to_string(a.nodes()).unwrap();
    // The tool is not the move tool and nothing is selected; the answer is the same, and nothing has moved.
    assert_eq!(a.hit(10.0, 10.0).as_deref(), Some("r"));
    assert_eq!(a.hit(500.0, 10.0), None);
    assert_eq!(serde_json::to_string(a.state()).unwrap(), before);
    assert_eq!(serde_json::to_string(a.nodes()).unwrap(), nodes_before);
    assert!(a.take_events().is_empty());
}

#[test]
fn hit_follows_the_document_including_what_is_not_confirmed_yet() {
    let mut a = app_of(
        json!([{"id": "r", "type": "rect", "pos": "a0", "x": 0, "y": 0, "w": 50, "h": 50}]),
        false,
    );
    assert_eq!(a.hit(10.0, 10.0).as_deref(), Some("r"));
    a.dispatch(Action::Move {
        id: "r".into(),
        x: 200.0,
        y: 0.0,
    });
    assert_eq!(a.hit(10.0, 10.0), None, "the pending move is the document");
    assert_eq!(a.hit(210.0, 10.0).as_deref(), Some("r"));
    a.dispatch(Action::Undo {});
    assert_eq!(
        a.hit(10.0, 10.0).as_deref(),
        Some("r"),
        "an undo puts it back"
    );
}

#[test]
fn hit_reads_the_laid_out_boxes_with_the_flag_on_and_the_stored_ones_with_it_off() {
    let list = json!([
        {"id": "f", "type": "frame", "pos": "a0", "w": 300, "h": 100, "layout": "row", "padding": 10, "clip": false},
        {"id": "b", "type": "rect", "parentId": "f", "pos": "a0", "x": 200, "y": 200, "w": 50, "h": 50},
    ]);
    let mut a = app_of(list, false);
    assert_eq!(
        a.hit(210.0, 210.0).as_deref(),
        Some("b"),
        "stored at (200, 200)"
    );
    assert_eq!(a.hit(20.0, 20.0).as_deref(), Some("f"));
    a.set_layout_on_canvas(true);
    assert_eq!(
        a.hit(20.0, 20.0).as_deref(),
        Some("b"),
        "the row puts it at (10, 10)"
    );
    assert_eq!(a.hit(210.0, 210.0), None);
}

#[test]
fn the_index_is_built_once_for_a_revision_and_again_after_a_change() {
    let mut a = app_of(
        json!([{"id": "r", "type": "rect", "pos": "a0", "x": 0, "y": 0, "w": 50, "h": 50}]),
        true,
    );
    assert_eq!(a.hit_builds(), 0, "nothing is built until someone asks");
    for i in 0..100 {
        a.hit(f64::from(i), 10.0);
    }
    assert_eq!(
        a.hit_builds(),
        1,
        "a hover over a file that has not changed builds once"
    );
    a.dispatch(Action::Move {
        id: "r".into(),
        x: 5.0,
        y: 5.0,
    });
    a.hit(10.0, 10.0);
    a.hit(11.0, 10.0);
    assert_eq!(a.hit_builds(), 2, "an edit builds again, once");
    a.set_layout_on_canvas(false);
    a.hit(10.0, 10.0);
    assert_eq!(a.hit_builds(), 3, "the flag changes the scene");
    a.dispatch(Action::SetTool { tool: Tool::Frame });
    a.dispatch(Action::Select {
        ids: vec!["r".into()],
    });
    a.hit(10.0, 10.0);
    assert_eq!(
        a.hit_builds(),
        3,
        "the tool and the selection are not the scene"
    );
}

/// Every text measures to the same size.
struct Fixed(f64, f64);

impl TextMeasurer for Fixed {
    fn measure(&self, _: &Node, _: Option<f64>) -> (f64, f64) {
        (self.0, self.1)
    }
}

#[test]
fn a_new_measurer_changes_what_a_hug_text_covers_and_builds_the_index_again() {
    let mut a = app_of(
        json!([
            {"id": "t", "type": "text", "pos": "a0", "text": "hello", "fontSize": 16, "sw": "hug", "sh": "hug"},
        ]),
        true,
    );
    a.set_measurer(Box::new(Fixed(10.0, 10.0)));
    assert_eq!(a.hit(50.0, 5.0), None);
    a.set_measurer(Box::new(Fixed(100.0, 10.0)));
    assert_eq!(a.hit(50.0, 5.0).as_deref(), Some("t"));
    assert_eq!(a.hit_builds(), 2);
}
