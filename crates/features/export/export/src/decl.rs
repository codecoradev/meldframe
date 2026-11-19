//! One node to its declarations: what the contract (`docs/fidelity-contract.md`, section 4) maps each field to.
//!
//! The first layer is the box: every element sits at its own x and y inside its parent (the exported frame at the
//! place the caller gives it) with its own width and height.

use crate::names::{escape, px};
use meldframe_protocol::Node;
use meldframe_render::{Paint, PaintedBox};

/// One CSS declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decl {
    pub prop: &'static str,
    pub value: String,
}

/// The declarations of a node: on the element, and on its `::after`.
#[derive(Debug, Default)]
pub(crate) struct Style {
    pub main: Vec<Decl>,
    pub after: Vec<Decl>,
}

fn d(prop: &'static str, value: impl Into<String>) -> Decl {
    Decl {
        prop,
        value: value.into(),
    }
}

/// The declarations of one node. `root` is the place of the exported frame: its own x and y when it is a top-level
/// frame, else the origin.
pub(crate) fn style(n: &Node, root: Option<(f64, f64)>) -> Style {
    let (x, y) = root.unwrap_or((n.x, n.y));
    Style {
        main: vec![
            d("position", "absolute"),
            d("left", px(x)),
            d("top", px(y)),
            d("width", px(n.w)),
            d("height", px(n.h)),
        ],
        after: Vec::new(),
    }
}

/// The tag and the attributes that name a node, for the element.
pub(crate) fn tag(b: &PaintedBox) -> &'static str {
    if b.paint == Paint::Text { "p" } else { "div" }
}

/// `data-node` and `data-name` of an element.
pub(crate) fn attrs(n: &Node) -> String {
    let mut a = format!("data-node=\"{}\"", escape(&n.id));
    if !n.name.is_empty() {
        a.push_str(&format!(" data-name=\"{}\"", escape(&n.name)));
    }
    a
}
