//! What the layout fields of the properties panel say about the node: the sides it has, whether its parent lays out, and
//! the settings that have no effect where it is (the rules of `docs/fidelity-contract.md`, G1 and the sizing table).

use meldframe_app_model::{Axis, LayoutNote, Sides};
use meldframe_layout::is_layout_frame;
use meldframe_protocol::{Node, NodeKind, Pad, SizeMode};

pub fn sides(p: Pad) -> Sides {
    Sides {
        top: p.top,
        right: p.right,
        bottom: p.bottom,
        left: p.left,
    }
}

/// The one value all four sides share, `None` when they differ.
pub fn uniform(p: Pad) -> Option<f64> {
    (p.top == p.right && p.right == p.bottom && p.bottom == p.left).then_some(p.top)
}

/// The settings of `n` that have no effect under `parent` (`None` for a node at the top of the file), in a fixed order:
/// width, then height, then margin, then absolute. The rules are those of the layout core: a node with a parent frame
/// is judged as `layout_findings` judges it (an absolute node is never a `fill_under_hug`), and an absolute node is
/// placed at its own `w` and `h` whatever its parent (`place.rs`), so a `fill` on it has no effect.
pub fn layout_notes(n: &Node, parent: Option<&Node>) -> Vec<LayoutNote> {
    let flows = parent.is_some_and(is_layout_frame);
    let mut out = Vec::new();
    for (axis, mode, parent_mode) in [
        (Axis::Width, n.sw, parent.map(|p| p.sw)),
        (Axis::Height, n.sh, parent.map(|p| p.sh)),
    ] {
        if mode == SizeMode::Fill && parent.is_some() {
            if n.absolute {
                out.push(LayoutNote::AbsoluteFillIgnored { axis });
            } else if !flows || parent_mode == Some(SizeMode::Hug) {
                // `fill` needs a parent that lays out and does not hug on that axis; the node keeps its own size otherwise.
                out.push(LayoutNote::FillUnderHug { axis });
            }
        }
        // `hug` means something for text and for a frame with auto layout; the rest is laid out as `fixed`.
        let hugs = n.kind == NodeKind::Text || is_layout_frame(n);
        if mode == SizeMode::Hug && !hugs {
            out.push(LayoutNote::HugIgnored { axis });
        }
    }
    // Margin and absolute mean something only in the flow of an auto layout frame: a node at the top of the file has
    // none, and an absolute node is out of the flow (`place.rs` puts it at its own offset, without the margin).
    let m = n.margin();
    if (!flows || n.absolute) && m.horizontal() + m.vertical() > 0.0 {
        out.push(LayoutNote::MarginIgnored {});
    }
    if n.absolute && !flows {
        out.push(LayoutNote::AbsoluteIgnored {});
    }
    out
}
