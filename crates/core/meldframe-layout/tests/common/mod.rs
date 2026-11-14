//! Shared helpers for the layout tests.
#![allow(dead_code)]

use meldframe_layout::{Rect, StubMeasurer, compute_rects};
use meldframe_protocol::Node;
use serde_json::{Value, json};

/// A node from the given fields; everything else takes the protocol defaults.
pub fn node(id: &str, kind: &str, parent: Option<&str>, fields: Value) -> Node {
    let mut v = json!({ "id": id, "type": kind, "parentId": parent });
    v.as_object_mut()
        .unwrap()
        .extend(fields.as_object().expect("fields are an object").clone());
    serde_json::from_value(v).expect("a valid node")
}

/// The rect of `id` after laying out `nodes` with the stub measurer.
pub fn rect_of(nodes: &[Node], id: &str) -> Rect {
    compute_rects(nodes, &StubMeasurer)
        .into_iter()
        .find(|(i, _)| i == id)
        .unwrap_or_else(|| panic!("no rect for {id}"))
        .1
}

pub fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}
