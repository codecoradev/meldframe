//! The preview of a drop and the drop agree. Over the two fixture files (an auto layout column with a free frame and a
//! row, and a row of frames, an empty column and a nest), every node dragged to every point of a coarse grid: when
//! `drop_preview` names a slot, the drop leaves the node in that frame at that index among its flow children; when it
//! names none, the drop does not leave the node in a flow. The grid is coarse (30 units) and its points land on
//! boxes, in gaps, in an empty frame and off every frame.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::App;
use meldframe_protocol::{LayoutMode, Node, NodeKind};
use std::collections::BTreeSet;

/// Whether the layout places the node: a child of an auto layout frame that is not absolute.
fn in_flow(a: &App, id: &str) -> bool {
    let n = node(a, id);
    n.parent_id
        .as_deref()
        .map(|p| node(a, p))
        .is_some_and(|p| !n.absolute && p.kind == NodeKind::Frame && p.layout != LayoutMode::None)
}

/// The children of `frame` that the layout places, in order.
fn flow(a: &App, frame: &str) -> Vec<String> {
    let mut k = kids(a, Some(frame));
    k.retain(|id| !node(a, id).absolute);
    k
}

/// Sweeps the grid for every node of the file; returns the frames that were a slot at least once.
fn sweep(list: Vec<Node>, xs: (f64, f64), ys: (f64, f64), step: f64) -> BTreeSet<String> {
    let ids: Vec<String> = list.iter().map(|n| n.id.clone()).collect();
    let mut slots = BTreeSet::new();
    let mut nones = 0;
    for id in &ids {
        let mut y = ys.0;
        while y <= ys.1 {
            let mut x = xs.0;
            while x <= xs.1 {
                // A fresh app for each point: the ops of the drops would pile up unconfirmed in one that goes on.
                let mut a = app_of(list.clone(), true);
                let (at, by) = ((x, y), (3.0, 4.0));
                let slot = a.drop_preview(id, at, by);
                drop_node(&mut a, id, at, by);
                match slot {
                    Some(s) => {
                        assert_eq!(
                            node(&a, id).parent_id.as_deref(),
                            Some(s.frame.as_str()),
                            "{id} at {at:?}: the frame"
                        );
                        assert_eq!(
                            flow(&a, &s.frame).iter().position(|k| k == id),
                            Some(s.index),
                            "{id} at {at:?}: the index in {}",
                            s.frame
                        );
                        slots.insert(s.frame);
                    }
                    None => {
                        nones += 1;
                        assert!(!in_flow(&a, id), "{id} at {at:?}: no slot, yet in a flow");
                    }
                }
                x += step;
            }
            y += step;
        }
    }
    assert!(nones > 0, "the grid has drops that go into no flow");
    slots
}

#[test]
fn the_slot_is_where_the_drop_puts_the_node_over_a_column_a_free_frame_and_a_row() {
    let slots = sweep(nodes(), (-20.0, 1050.0), (-20.0, 340.0), 30.0);
    assert!(slots.contains("f") && slots.contains("r"), "{slots:?}");
}

#[test]
fn the_slot_is_where_the_drop_puts_the_node_over_a_row_of_frames_an_empty_frame_and_a_nest() {
    let slots = sweep(cards(), (-20.0, 760.0), (-20.0, 720.0), 30.0);
    assert!(
        slots.contains("deck") && slots.contains("empty") && slots.contains("y"),
        "{slots:?}"
    );
}

#[test]
fn a_file_of_a_few_thousand_nodes_gives_the_same_answer() {
    // One column with 1,500 flow children, and a free frame beside it.
    let mut list = vec![serde_json::json!(
        {"id": "col", "type": "frame", "pos": "a0", "w": 100, "h": 40000, "layout": "column"}
    )];
    list.push(serde_json::json!({"id": "free", "type": "frame", "pos": "a1", "x": 500, "w": 100, "h": 100}));
    for i in 0..1500 {
        list.push(serde_json::json!(
            {"id": format!("k{i}"), "type": "rect", "parentId": "col", "pos": format!("a{i:05}"), "w": 50, "h": 10}
        ));
    }
    let nodes: Vec<Node> = serde_json::from_value(serde_json::Value::Array(list)).unwrap();
    let mut a = app_of(nodes, true);
    // Past the middle of about 750 children (each 10 high, 0 gap): a slot, and the drop agrees.
    let at = (10.0, 7500.0);
    let slot = a.drop_preview("k3", at, (0.0, 0.0)).expect("a slot");
    drop_node(&mut a, "k3", at, (0.0, 0.0));
    assert_eq!(
        flow(&a, "col").iter().position(|k| k == "k3"),
        Some(slot.index)
    );
    // Off every frame: no slot.
    assert!(a.drop_preview("k3", (5000.0, 5000.0), (0.0, 0.0)).is_none());
}
