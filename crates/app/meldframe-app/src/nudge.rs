//! What an arrow key does to the selection: each selected node goes the way its place allows.
//!
//! A pure function of the nodes, the selection, the key and the host flag, like `arrow` and `drop`: no state, no clock,
//! nothing of the front end.
//!
//! The rules (E8.1.1):
//!
//! - A node outside a flow moves by one step along the axis of the key, in world space whatever the zoom: 1 unit, or 10 when
//!   the step is coarse (Shift on the key table). Up and left are negative. Only the axis that moves is written, so a
//!   collaborator's edit of the other one is not overwritten. A step past the limit of a coordinate (`max_coord`) is refused
//!   with the whole press, as `MoveBy` is.
//! - A child of an auto layout frame is in the flow, and the layout owns its place: with the host flag on it never gets a
//!   stored `x` or `y`. A plain press moves it one place along the flow (`meldframe_app_rules::arrow`); a coarse press does nothing to it. With the
//!   flag off the canvas draws the stored boxes and every node moves, a flow child included, as a drag does.
//! - A selected node under a selected node that moves (one outside a flow) is carried along by it: it is not moved twice, and
//!   a selected flow child of a moving frame is not put in another place either. One under a flow child that stays is acted on
//!   alone. The press acts on the selected nodes that no moving selected node carries.
//! - Every press is one history step (the app applies the commands as one group): there is no clock in the app layer to
//!   gather a held key into one, so a held key is not an editor key (`keys`), and a person who wants 10 uses Shift.

use crate::parts::document::{apply_all, validate};
use meldframe_app_model::Direction;
use meldframe_app_rules::arrow::arrow_commands;
use meldframe_layout::is_layout_frame;
use meldframe_protocol::{Command, Node, NodePatch};
use meldframe_sync::{Replica, SubmitError};
use std::collections::{HashMap, HashSet};

/// The step of a plain press, in the units of the file.
pub const STEP: f64 = 1.0;
/// The step of a coarse press.
pub const COARSE_STEP: f64 = 10.0;

/// The commands of one press of the arrow key `key` for the nodes `selected`; none when nothing moves.
pub fn nudge_commands(
    nodes: &[Node],
    selected: &[String],
    key: Direction,
    coarse: bool,
    laid_out: bool,
) -> Vec<Command> {
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let in_flow = |n: &Node| {
        laid_out
            && !n.absolute
            && n.parent_id
                .as_deref()
                .and_then(|p| by_id.get(p))
                .is_some_and(|p| is_layout_frame(p))
    };
    let moving: HashSet<&str> = selected
        .iter()
        .filter_map(|id| by_id.get(id.as_str()))
        .filter(|n| !in_flow(n))
        .map(|n| n.id.as_str())
        .collect();
    // A parent chain longer than the file is a cycle: stop.
    let follows = |n: &Node| {
        let mut cur = n.parent_id.as_deref().and_then(|p| by_id.get(p));
        for _ in 0..=nodes.len() {
            match cur {
                Some(p) if moving.contains(p.id.as_str()) => return true,
                Some(p) => cur = p.parent_id.as_deref().and_then(|q| by_id.get(q)),
                None => return false,
            }
        }
        false
    };
    let mut seen = HashSet::new();
    let acting: Vec<&Node> = selected
        .iter()
        .filter_map(|id| by_id.get(id.as_str()).copied())
        .filter(|n| !follows(n) && seen.insert(n.id.as_str()))
        .collect();
    let mut out = if laid_out && !coarse {
        let ids: Vec<String> = acting.iter().map(|n| n.id.clone()).collect();
        arrow_commands(nodes, &ids, key)
    } else {
        Vec::new()
    };
    let step = if coarse { COARSE_STEP } else { STEP };
    let (dx, dy) = match key {
        Direction::Up => (0.0, -step),
        Direction::Down => (0.0, step),
        Direction::Left => (-step, 0.0),
        Direction::Right => (step, 0.0),
    };
    for n in acting.iter().filter(|n| moving.contains(n.id.as_str())) {
        out.push(Command::UpdateNode {
            id: n.id.clone(),
            patch: NodePatch {
                x: (dx != 0.0).then_some(n.x + dx),
                y: (dy != 0.0).then_some(n.y + dy),
                ..Default::default()
            },
        });
    }
    out
}

/// What a press is: the arrow key, whether it is coarse (Shift), and the host flag `layout_on_canvas`.
pub type Press = (Direction, bool, bool);

/// The commands of a press, checked as the replica would take them (a refusal is the error of the whole press).
fn checked(
    r: &Replica,
    selected: &[String],
    (key, coarse, laid_out): Press,
) -> Result<Vec<Command>, SubmitError> {
    let cmds = nudge_commands(r.nodes(), selected, key, coarse, laid_out);
    validate(r, &cmds)?;
    Ok(cmds)
}

/// One press of an arrow key applied to the replica as one history step; nothing when nothing moves.
pub fn nudge(r: &mut Replica, selected: &[String], press: Press) -> Result<(), SubmitError> {
    let cmds = checked(r, selected, press)?;
    apply_all(r, &cmds)
}

/// Whether that press would move anything and be taken: what the enabled state of a button that sends it asks.
pub fn can_nudge(r: &Replica, selected: &[String], press: Press) -> bool {
    checked(r, selected, press).is_ok_and(|cmds| !cmds.is_empty())
}
