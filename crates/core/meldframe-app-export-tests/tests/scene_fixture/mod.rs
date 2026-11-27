//! The file the scene epoch tests read: a column `f` (padding 10, gap 5) with the rectangles `a` and `c`.
#![allow(dead_code)]

use meldframe_app::{Action, App};
use meldframe_protocol::Node;
use serde_json::json;

pub fn nodes() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "f", "type": "frame", "pos": "a0", "w": 200, "h": 400, "layout": "column", "padding": 10, "gap": 5},
        {"id": "a", "type": "rect", "parentId": "f", "pos": "a0", "w": 50, "h": 20},
        {"id": "c", "type": "rect", "parentId": "f", "pos": "a1", "w": 50, "h": 40},
    ]))
    .unwrap()
}

pub fn app(layout: bool) -> App {
    app_of(nodes(), layout)
}

pub fn app_of(nodes: Vec<Node>, layout: bool) -> App {
    let mut a = App::new("user:me", "c0", 1, nodes, 0).unwrap();
    a.set_layout_on_canvas(layout);
    a.dispatch(Action::Connected { now_ms: 0 });
    a.take_events();
    a
}

/// The height of the box the canvas draws for `id`.
pub fn drawn_height(a: &App, id: &str) -> f64 {
    a.scene().iter().find(|b| b.id == id).expect("a box").h
}
