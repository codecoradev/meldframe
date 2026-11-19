//! E3.1: the painted boxes of shared fixtures. `tests/fixtures/scene.json` holds node lists and the boxes
//! `resolve` gives for them, as the wire JSON the canvas reads. The web tests read the same file, so the
//! canvas shows what this step says (`packages/web-link/src/scene.test.ts`).

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_protocol::Node;
use meldframe_render::resolve;
use serde_json::{Value, json};

const FIXTURES: &str = include_str!("fixtures/scene.json");

#[test]
fn every_fixture_resolves_to_its_boxes() {
    let cases: Vec<Value> = serde_json::from_str(FIXTURES).unwrap();
    assert!(cases.len() >= 8);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let nodes: Vec<Node> = serde_json::from_value(case["nodes"].clone()).unwrap();
        let boxes = serde_json::to_value(resolve(&nodes)).unwrap();
        assert_eq!(boxes, case["boxes"], "{name}");
    }
}

#[test]
fn a_box_is_plain_json_with_camel_case_names() {
    let n: Node = serde_json::from_value(json!({
        "id": "n", "type": "rect", "parentId": "p", "x": 1, "y": 2, "w": 3, "h": 4, "fill": "#102030",
        "gradient": {"kind": "linear", "angle": 90, "stops": [{"pos": 0, "color": "#fff"}, {"pos": 1, "color": "#000"}]},
        "image": {"assetId": "a", "fit": "fill"}
    }))
    .unwrap();
    let p: Node = serde_json::from_value(json!({"id": "p", "type": "frame"})).unwrap();
    let boxes = serde_json::to_value(resolve(&[n, p])).unwrap();
    assert_eq!(
        boxes[1],
        json!({
            "id": "n", "parent": "p", "paint": "rect", "x": 1.0, "y": 2.0, "w": 3.0, "h": 4.0, "fill": 0x102030,
            "radius": 0.0, "text": "", "fontSize": 0.0, "name": "", "clip": true, "opacity": 1.0,
            "gradient": {"kind": "linear", "angle": 90.0, "stops": [[0.0, "#fff"], [1.0, "#000"]]},
            "image": {"assetId": "a", "fit": "fill"}
        })
    );
    assert_eq!(boxes[0]["parent"], Value::Null);
    assert_eq!(boxes[0]["paint"], "frame");
    assert!(boxes[0].get("gradient").is_none() && boxes[0].get("image").is_none());
}
