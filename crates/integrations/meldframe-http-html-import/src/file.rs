//! What a call needs from the file: where the root goes, and the check a dry run makes in place of the write.

use crate::plan::Planned;
use meldframe_core::{Limits, apply_doc};
use meldframe_host::Refusal;
use meldframe_http::{Failure, Reply};
use meldframe_protocol::{Command, Doc, Node};

/// Put the root where the call says: at the origin of `parent`, else 40 px to the right of the rightmost top-level
/// node (at the top, `y` 0, and at 0 in an empty file). A root that the file already holds, because an earlier try of
/// this call wrote part of it, keeps the place it has, so the retry is the same command.
pub fn place(p: &mut Planned, parent: Option<&str>, nodes: &[Node]) {
    let root = p.root.clone();
    let Some(Command::AddNode { node, .. }) = p.plan.commands.first_mut() else {
        return;
    };
    node.parent_id = parent.map(str::to_owned);
    (node.x, node.y) = match parent {
        Some(_) => (0.0, 0.0),
        None => {
            let right = (nodes.iter().filter(|n| n.parent_id.is_none()))
                .map(|n| n.x + n.w)
                .reduce(f64::max);
            (right.map_or(0.0, |r| r + meldframe_variant::GAP), 0.0)
        }
    };
    if let Some(have) = nodes.iter().find(|n| n.id == root) {
        (node.x, node.y) = (have.x, have.y);
    }
}

/// Apply the commands in order to a copy of the file, under the limits every write is checked against: the refusal
/// of the first that fails, worded as the write would word it (code, message and `index`).
pub fn dry_run(doc: Doc, commands: &[Command]) -> Result<(), Reply> {
    let (mut doc, limits) = (doc, Limits::default());
    for (index, c) in commands.iter().enumerate() {
        if let Err(e) = apply_doc(&mut doc, c, &limits) {
            let refusal = Refusal::Apply(e);
            return Err(Reply::failure(&Failure::Batch { index, refusal }));
        }
    }
    Ok(())
}
