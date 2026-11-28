//! The world rectangle of every painted box, in box order (`world_rects`): the one placement the hit rule and a painter read,
//! worked out in one pass. The tests compare it with `HitIndex::rect_of`, which asks the same index one id at a time.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_protocol::Node;
use meldframe_render::{HitIndex, PaintedBox, resolve, world_rects};

fn boxes_of(nodes: &str) -> Vec<PaintedBox> {
    let nodes: Vec<Node> = serde_json::from_str(nodes).unwrap();
    resolve(&nodes)
}

#[test]
fn a_box_is_placed_at_the_sum_of_the_offsets_of_its_ancestors() {
    let boxes = boxes_of(
        r#"[
          {"id": "root", "type": "frame", "x": 5, "y": 6, "w": 800, "h": 600},
          {"id": "f1", "type": "frame", "parentId": "root", "x": 100, "y": 100, "w": 300, "h": 200},
          {"id": "r1", "type": "rect", "parentId": "f1", "x": -10, "y": 10, "w": 50, "h": 40}
        ]"#,
    );
    let rects = world_rects(&boxes);
    assert_eq!(rects.len(), boxes.len());
    let at = |id: &str| {
        let i = boxes.iter().position(|b| b.id == id).unwrap();
        rects[i].map(|r| (r.x, r.y, r.w, r.h))
    };
    assert_eq!(at("root"), Some((5.0, 6.0, 800.0, 600.0)));
    assert_eq!(at("f1"), Some((105.0, 106.0, 300.0, 200.0)));
    assert_eq!(at("r1"), Some((95.0, 116.0, 50.0, 40.0)));
}

#[test]
fn a_repeated_id_has_no_rectangle_because_the_index_leaves_it_out() {
    let mut boxes = boxes_of(
        r#"[
          {"id": "root", "type": "frame", "x": 0, "y": 0, "w": 800, "h": 600},
          {"id": "r1", "type": "rect", "parentId": "root", "x": 10, "y": 10, "w": 50, "h": 40}
        ]"#,
    );
    let mut again = boxes[1].clone();
    again.x = 300.0;
    boxes.push(again);

    let rects = world_rects(&boxes);
    assert!(rects[1].is_some());
    assert!(
        rects[2].is_none(),
        "the second box of an id is the one the index drops"
    );
    let index = HitIndex::from_boxes(&boxes);
    assert_eq!(index.len(), 2);
    assert_eq!(index.rect_of("r1").map(|r| r.x), Some(10.0));
}

#[test]
fn five_thousand_boxes_are_placed_as_the_index_places_them() {
    let mut nodes = vec![
        r#"{"id": "root", "type": "frame", "x": 0, "y": 0, "w": 9000, "h": 9000}"#.to_string(),
    ];
    for f in 0..50 {
        nodes.push(format!(
            r#"{{"id": "f{f}", "type": "frame", "parentId": "root", "x": {}, "y": {}, "w": 100, "h": 100}}"#,
            f * 7,
            f * 3
        ));
        for r in 0..99 {
            nodes.push(format!(
                r#"{{"id": "r{f}-{r}", "type": "rect", "parentId": "f{f}", "x": {r}, "y": {}, "w": 5, "h": 5}}"#,
                r * 2
            ));
        }
    }
    let boxes = boxes_of(&format!("[{}]", nodes.join(",")));
    assert_eq!(boxes.len(), 5001);
    let rects = world_rects(&boxes);
    let index = HitIndex::from_boxes(&boxes);
    // The index answers one id at a time with a scan, so ask it about a sample of the boxes, not about all of them.
    for i in (0..boxes.len()).step_by(97) {
        assert_eq!(rects[i], index.rect_of(&boxes[i].id), "box {}", boxes[i].id);
    }
    assert_eq!(
        rects[boxes.len() - 1].map(|r| (r.x, r.y)),
        Some((343.0 + 98.0, 147.0 + 196.0))
    );
}
