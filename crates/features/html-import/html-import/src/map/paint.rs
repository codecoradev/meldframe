//! THE PAINT SEAM. Slice 5 (`paint/*`) reads the paint of a computed style into the paint fields of a Node; until it
//! lands this file is the one place that stands in for it, and nothing here reads a paint property. To join the two,
//! replace [`Paint`] by the type of `paint::read` and the body of [`paint_of`] by a call to it; [`put`] and [`decorates`]
//! copy or compare its fields, so they are the only other lines that name them.
//!
//! The mapping needs from a paint, besides the Node's own paint fields:
//!
//! - `ellipse`: the node is an ellipse (a `border-radius: 50%` with no children);
//! - `border_room`: the width a CSS border takes from the padding on each side (0 for the export's own `::after`
//!   border, which takes no room), added by the mapping to the padding and, under `box-sizing: content-box`, to a
//!   fixed size.
//!
//! `Paint::default()` is nothing painted: no fill (`none`), no radius, no border, no shadow, opaque.

use html_import_tree::Report;
use meldframe_protocol::{Bind, Border, Gradient, ImageFill, Node, Shadow};

use crate::css::Style;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Paint {
    pub fill: Bind<String>,
    pub radius: Bind<f64>,
    pub opacity: Bind<f64>,
    pub border: Border,
    pub border_room: f64,
    pub shadows: Vec<Shadow>,
    pub gradient: Option<Gradient>,
    pub image: Option<ImageFill>,
    pub ellipse: bool,
}

impl Default for Paint {
    fn default() -> Self {
        Paint {
            fill: Bind::Lit("none".into()),
            radius: Bind::Lit(0.0),
            opacity: Bind::Lit(1.0),
            border: Border::default(),
            border_room: 0.0,
            shadows: Vec::new(),
            gradient: None,
            image: None,
            ellipse: false,
        }
    }
}

/// The paint of one element. Placeholder: nothing is read (see the module).
pub(crate) fn paint_of(style: &Style, report: &mut Report) -> Paint {
    let _ = (style, report);
    Paint::default()
}

/// Write the paint fields into a node. A text takes only the opacity: it paints no border, gradient, image or shadow.
pub(crate) fn put(node: &mut Node, p: &Paint) {
    node.opacity = p.opacity.clone();
    if node.kind == meldframe_protocol::NodeKind::Text {
        return;
    }
    node.fill = p.fill.clone();
    node.radius = p.radius.clone();
    node.border = p.border.clone();
    node.shadows = p.shadows.clone();
    node.gradient = p.gradient.clone();
    node.image = p.image.clone();
}

/// Does the element paint a box (so that it is a frame with a text child, not a text)? Everything but the opacity,
/// which a text has too.
pub(crate) fn decorates(p: &Paint) -> bool {
    let plain = Paint {
        opacity: p.opacity.clone(),
        ..Paint::default()
    };
    *p != plain
}
