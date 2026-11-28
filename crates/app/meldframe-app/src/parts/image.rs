//! The rules of the image picker of the Fill section: when it may open on a node, and what choosing an image does to the node.
//! Whether an asset exists is not decided here: the server is the authority, and the canvas reports `asset_not_found`.
//! The shape of the id and the kinds that take an image are the core's checks, run when the command is applied
//! (`invalid_asset_id`, `image_on_text` in the app layer's words).

use meldframe_protocol::{ImageFill, ImageFit, Node, NodeKind, NodePatch};

/// The code a request to open the picker for node `id` is refused with, when it is: no such node, the node is not the one
/// selected, or it is text, which takes no image.
pub fn open_refusal(nodes: &[Node], selection: &[String], id: &str) -> Option<&'static str> {
    match nodes.iter().find(|n| n.id == id) {
        None => Some("not_found"),
        Some(_) if !matches!(selection, [only] if only == id) => Some("not_selected"),
        Some(n) if n.kind == NodeKind::Text => Some("image_on_text"),
        Some(_) => None,
    }
}

/// Whether the picker open for node `id` still has what it needs: the node is there and is the one selected.
pub fn is_open_valid(nodes: &[Node], selection: &[String], id: &str) -> bool {
    matches!(selection, [only] if only == id) && nodes.iter().any(|n| n.id == id)
}

/// The patch that sets the image of a node, `None` when the node has this image and fit already (no change, no undo step).
/// Without a `fit` the node keeps the fit it has, and cover is the fit of a node that has no image. The colour and the
/// gradient under the image are left as they are.
pub fn image_fill_patch(node: &Node, asset_id: String, fit: Option<ImageFit>) -> Option<NodePatch> {
    let fit = fit
        .or_else(|| node.image.as_ref().map(|i| i.fit))
        .unwrap_or_default();
    let image = ImageFill { asset_id, fit };
    if node.image.as_ref() == Some(&image) {
        return None;
    }
    Some(NodePatch {
        image: Some(Some(image)),
        ..NodePatch::default()
    })
}
