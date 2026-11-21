//! The fixtures and helpers of the layout interaction tests (`layout_drop.rs`, `layout_resize.rs`).
#![allow(dead_code)]

use meldframe_app::{Action, App, Effect, Event, Properties};
use meldframe_protocol::{Command, Node};
use serde_json::json;

/// `f`: a column (padding 10, gap 5) with `a` (y 10 to 30), `b` (35 to 65) and `c` (70 to 110), all at x 10, and
/// `abs`, an absolute child at its own (150, 200). `g`: a free frame at x 400 with `fg` at its own offset (20, 20).
/// `r`: a row at x 700, with the default padding of 16, with `r1` (716 to 756) and `r2` (766 to 806).
pub fn nodes() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "f", "type": "frame", "pos": "a0", "w": 200, "h": 300, "layout": "column", "padding": 10, "gap": 5},
        {"id": "a", "type": "rect", "parentId": "f", "pos": "a0", "w": 50, "h": 20},
        {"id": "b", "type": "rect", "parentId": "f", "pos": "a1", "w": 50, "h": 30},
        {"id": "c", "type": "rect", "parentId": "f", "pos": "a2", "w": 50, "h": 40},
        {"id": "abs", "type": "rect", "parentId": "f", "pos": "a3", "x": 150, "y": 200, "w": 20, "h": 20, "absolute": true},
        {"id": "g", "type": "frame", "pos": "a1", "x": 400, "w": 200, "h": 200},
        {"id": "fg", "type": "rect", "parentId": "g", "pos": "a0", "x": 20, "y": 20, "w": 30, "h": 30},
        {"id": "r", "type": "frame", "pos": "a2", "x": 700, "w": 300, "h": 100, "layout": "row", "gap": 10},
        {"id": "r1", "type": "rect", "parentId": "r", "pos": "a0", "w": 40, "h": 40},
        {"id": "r2", "type": "rect", "parentId": "r", "pos": "a1", "w": 40, "h": 40},
    ]))
    .unwrap()
}

/// `b` replaced by a rectangle with the width sized `mode` and a hug height.
pub fn with_sized_b(mode: &str) -> Vec<Node> {
    let mut v = nodes();
    let b = v.iter_mut().find(|n| n.id == "b").unwrap();
    *b = serde_json::from_value(json!({
        "id": "b", "type": "rect", "parentId": "f", "pos": "a1", "w": 1, "h": 30, "sw": mode, "sh": "hug"
    }))
    .unwrap();
    v
}

pub fn app_of(list: Vec<Node>, on: bool) -> App {
    let mut a = App::new("user:me", "c0", 1, list, 0).unwrap();
    a.set_layout_on_canvas(on);
    a.dispatch(Action::Connected { now_ms: 0 });
    a.take_events();
    a
}

pub fn app(on: bool) -> App {
    app_of(nodes(), on)
}

pub fn node(a: &App, id: &str) -> Node {
    a.nodes().iter().find(|n| n.id == id).cloned().unwrap()
}

/// The children of `parent` in sibling order.
pub fn kids(a: &App, parent: Option<&str>) -> Vec<String> {
    let mut k: Vec<&Node> = a
        .nodes()
        .iter()
        .filter(|n| n.parent_id.as_deref() == parent)
        .collect();
    k.sort_by(|x, y| x.pos.cmp(&y.pos).then(x.id.cmp(&y.id)));
    k.into_iter().map(|n| n.id.clone()).collect()
}

/// Drops `id` with the pointer at `at` after a drag of `by`; returns the commands that were sent.
pub fn drop_node(a: &mut App, id: &str, at: (f64, f64), by: (f64, f64)) -> Vec<Command> {
    let fx = a.dispatch(Action::DropNode {
        id: id.into(),
        x: at.0,
        y: at.1,
        dx: by.0,
        dy: by.1,
    });
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.command.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

/// Drops the selection `ids` with the pointer at `at` after a drag of `by`; returns the commands that were sent.
pub fn drop_nodes(a: &mut App, ids: &[&str], at: (f64, f64), by: (f64, f64)) -> Vec<Command> {
    let fx = a.dispatch(Action::DropNodes {
        ids: ids.iter().map(|s| s.to_string()).collect(),
        x: at.0,
        y: at.1,
        dx: by.0,
        dy: by.1,
    });
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.command.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

pub fn refusals(a: &mut App) -> Vec<String> {
    a.take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect()
}

pub fn steps(a: &App) -> usize {
    a.state().history.undo_steps
}

pub fn box_of(a: &App, id: &str) -> (f64, f64, f64, f64) {
    let scene = a.scene();
    let b = scene.iter().find(|b| b.id == id).expect("a box");
    (b.x, b.y, b.w, b.h)
}

pub fn properties_of(a: &mut App, id: &str) -> Properties {
    a.dispatch(Action::Select {
        ids: vec![id.to_string()],
    });
    a.state().properties.clone().expect("properties")
}

/// `deck`: a row at (100, 50), padding 10, gap 10, with the free frames `c1` (x 110 to 210), `c2` (220 to 320, holding
/// the rectangle `k`) and `c3` (330 to 430), all from y 60 to 160. `other`: a free frame at (100, 400). `empty`: a column
/// at (500, 400) with no child. `tree`: a free frame at (100, 500) holding the free frame `x` (20, 20, 200 x 200), which
/// holds the free frame `m` (10, 10), which holds the column `y` (5, 5, 100 x 100): `y` is at world (135, 535).
pub fn cards() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "deck", "type": "frame", "pos": "a0", "x": 100, "y": 50, "w": 600, "h": 200, "layout": "row", "padding": 10, "gap": 10},
        {"id": "c1", "type": "frame", "parentId": "deck", "pos": "a0", "w": 100, "h": 100},
        {"id": "c2", "type": "frame", "parentId": "deck", "pos": "a1", "w": 100, "h": 100},
        {"id": "k", "type": "rect", "parentId": "c2", "pos": "a0", "x": 10, "y": 10, "w": 20, "h": 20},
        {"id": "c3", "type": "frame", "parentId": "deck", "pos": "a2", "w": 100, "h": 100},
        {"id": "other", "type": "frame", "pos": "a1", "x": 100, "y": 400, "w": 300, "h": 100},
        {"id": "empty", "type": "frame", "pos": "a2", "x": 500, "y": 400, "w": 200, "h": 100, "layout": "column"},
        {"id": "tree", "type": "frame", "pos": "a3", "x": 100, "y": 500, "w": 400, "h": 300},
        {"id": "x", "type": "frame", "parentId": "tree", "pos": "a0", "x": 20, "y": 20, "w": 200, "h": 200},
        {"id": "m", "type": "frame", "parentId": "x", "pos": "a0", "x": 10, "y": 10, "w": 150, "h": 150},
        {"id": "y", "type": "frame", "parentId": "m", "pos": "a0", "x": 5, "y": 5, "w": 100, "h": 100, "layout": "column"},
    ]))
    .unwrap()
}
