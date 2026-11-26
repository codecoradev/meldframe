//! Pieces of the document actions that read no state: the command that updates a node, the node a create action
//! makes, and the stable code of a refusal.

use meldframe_app_model::{CreateKind, FramePreset, History, OutOp};
use meldframe_check::{ApplyError, Limits, check_command_with};
use meldframe_protocol::{Command, Node, NodePatch, Variables};
use meldframe_sync::{Outgoing, Replica, SubmitError};
use meldframe_variant::{VariantError, variant_of};
use serde_json::json;

pub fn update(id: &str, patch: NodePatch) -> Command {
    Command::UpdateNode {
        id: id.to_string(),
        patch,
    }
}

/// The command that moves a node to `(x, y)`.
pub fn move_to(id: &str, x: f64, y: f64) -> Command {
    let patch = NodePatch {
        x: Some(x),
        y: Some(y),
        ..Default::default()
    };
    update(id, patch)
}

/// The command that sets the text of a node.
pub fn edit_text(id: &str, text: String) -> Command {
    let patch = NodePatch {
        text: Some(text),
        ..Default::default()
    };
    update(id, patch)
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
        ApplyError::ImageOnText => "image_on_text",
        ApplyError::InvalidAssetId => "invalid_asset_id",
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
        CreateKind::Ellipse => "ellipse",
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

/// The frame a preset makes, named as the preset is.
pub fn preset_node(id: &str, preset: FramePreset, x: f64, y: f64, parent: Option<String>) -> Node {
    let (w, h) = preset.size();
    let mut node = new_node(id, CreateKind::Frame, x, y, parent, Some(w), Some(h));
    node.name = preset.name().into();
    node
}

/// A refusal as the app words it: variable errors keep their `E_` code (`docs/error-codes.md`).
pub fn variable_code(e: &ApplyError) -> Option<&'static str> {
    match e {
        ApplyError::Variable(v) => Some(v.code()),
        _ => None,
    }
}

/// Apply the commands as one history group (one undo step), stopping at the first that fails.
pub fn apply_all(r: &mut Replica, cmds: &[Command]) -> Result<(), SubmitError> {
    r.begin_group();
    let done = cmds.iter().try_for_each(|c| r.apply(c));
    r.end_group();
    done
}

/// The ops the replica queued, as the effect that sends them carries them.
pub fn out_ops(ops: Vec<Outgoing>) -> Vec<OutOp> {
    ops.into_iter()
        .map(|o| OutOp {
            key: o.key,
            command: o.command,
            deps: o.deps,
        })
        .collect()
}

/// The commands that move each selected node that the document has by `(dx, dy)`.
pub fn move_by(selection: &[String], nodes: &[Node], dx: f64, dy: f64) -> Vec<Command> {
    selection
        .iter()
        .filter_map(|id| nodes.iter().find(|n| n.id == *id))
        .map(|n| {
            update(
                &n.id,
                NodePatch {
                    x: Some(n.x + dx),
                    y: Some(n.y + dy),
                    ..Default::default()
                },
            )
        })
        .collect()
}

/// Every command is valid on the current document, checked one by one against it (the caller has checked the
/// queue has room): a list that would fail half way is refused whole.
pub fn check_all(nodes: &[Node], vars: &Variables, cmds: &[Command]) -> Result<(), SubmitError> {
    let limits = Limits::default();
    cmds.iter()
        .try_for_each(|c| check_command_with(nodes, vars, c, &limits))
        .map_err(SubmitError::Invalid)
}

/// What a request for a variant of a frame comes to.
#[derive(Debug)]
pub enum Variant {
    /// The commands of the copy, which apply as one step, and the id of its root.
    Made {
        commands: Vec<Command>,
        root: String,
    },
    /// The node is not a frame: a refusal with the code `not_a_frame`.
    NotAFrame,
}

/// The variant of frame `id`: its subtree copied with ids made from `prefix` and the counter `next_id`, which moves on
/// (the whole copy was applied to a copy of the file already, so it applies to the replica as one step).
pub fn make_variant(
    nodes: &[Node],
    vars: &Variables,
    id: &str,
    prefix: &str,
    next_id: &mut u64,
) -> Result<Variant, SubmitError> {
    let next = || {
        *next_id += 1;
        format!("{prefix}-n{next_id}")
    };
    match variant_of(nodes, vars, id, next, &Limits::default()) {
        Ok(v) => Ok(Variant::Made {
            commands: v.commands,
            root: v.root,
        }),
        Err(VariantError::Apply(e)) => Err(e.into()),
        Err(VariantError::NotFound) => Err(ApplyError::NotFound(id.to_string()).into()),
        Err(VariantError::NotAFrame) => Ok(Variant::NotAFrame),
    }
}

/// The undo and redo buttons of a replica.
pub fn history_of(r: &Replica) -> History {
    History {
        can_undo: r.can_undo(),
        can_redo: r.can_redo(),
        undo_steps: r.undo_depth(),
        redo_steps: r.redo_depth(),
    }
}

/// The node an unconfirmed op `key` removes, when it is a removal.
pub fn removal_of(unconfirmed: Vec<Outgoing>, key: &str) -> Option<String> {
    unconfirmed
        .into_iter()
        .find(|o| o.key == key)
        .and_then(|o| match o.command {
            Command::RemoveNode { id } => Some(id),
            _ => None,
        })
}
