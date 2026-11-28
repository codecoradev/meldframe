//! Which nodes a marquee selects.

use meldframe_protocol::Node;
use std::collections::HashSet;

/// The ids among `overlapped` (a marquee's boxes, back to front) that are top-level nodes: those with no parent, which are the
/// children of the page. A marquee selects what a person sees as the objects of the page; the nodes inside a frame are reached by
/// a click or the layers panel, so that sweeping over a frame never selects the frame and its whole content.
pub fn top_level<'a>(nodes: &[Node], overlapped: impl Iterator<Item = &'a str>) -> Vec<String> {
    let roots: HashSet<&str> = nodes
        .iter()
        .filter(|n| n.parent_id.is_none())
        .map(|n| n.id.as_str())
        .collect();
    overlapped
        .filter(|id| roots.contains(id))
        .map(str::to_owned)
        .collect()
}
