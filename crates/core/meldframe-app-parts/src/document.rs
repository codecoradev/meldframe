//! Pieces of the document actions that read no state: the command that updates a node, the node a create action
//! makes, and the stable code of a refusal.

use meldframe_app_model::CreateKind;
use meldframe_check::ApplyError;
use meldframe_protocol::{Command, Node, NodePatch};
use meldframe_sync::SubmitError;
use serde_json::json;

pub fn update(id: &str, patch: NodePatch) -> Command {
    Command::UpdateNode {
        id: id.to_string(),
        patch,
    }
}

/// A stable code for a refusal; the front end words the message.
pub fn code(e: &SubmitError) -> &'static str {
    let SubmitError::Invalid(e) = e else {
        return "outbox_full";
    };
    if let Some(code) = variable_code(e) {
        return code;
    }
    match e {
        ApplyError::NotFound(_) => "not_found",
        ApplyError::DuplicateId(_) => "duplicate_id",
        ApplyError::ParentNotFound(_) | ApplyError::ParentNotFrame(_) => "bad_parent",
        ApplyError::Cycle => "cycle",
        ApplyError::GradientOnText => "gradient_on_text",
        ApplyError::TooManyNodes(_)
        | ApplyError::TooDeep(_)
        | ApplyError::TooLong { .. }
        | ApplyError::TooMany { .. }
        | ApplyError::PosTooLong { .. } => "limit",
        _ => "invalid",
    }
}

pub fn new_node(
    id: &str,
    kind: CreateKind,
    x: f64,
    y: f64,
    parent: Option<String>,
    w: Option<f64>,
    h: Option<f64>,
) -> Node {
    let kind = match kind {
        CreateKind::Frame => "frame",
        CreateKind::Rectangle => "rect",
        CreateKind::Text => "text",
    };
    let mut node = json!({"id": id, "type": kind, "parentId": parent, "x": x, "y": y});
    // Sizes are finite here (checked by the caller): `json!` would turn a NaN into null.
    if let Some(w) = w {
        node["w"] = json!(w);
    }
    if let Some(h) = h {
        node["h"] = json!(h);
    }
    serde_json::from_value(node).expect("a node with these fields is valid")
}

/// A refusal as the app words it: variable errors keep their `E_` code (`docs/error-codes.md`).
pub fn variable_code(e: &ApplyError) -> Option<&'static str> {
    match e {
        ApplyError::Variable(v) => Some(v.code()),
        _ => None,
    }
}
