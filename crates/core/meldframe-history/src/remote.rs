//! Another actor's command: it is applied without a history step, and my history is rewritten against
//! it so that undo and redo never overwrite what the other actor did after me. The rules are in
//! `meldframe_undo::History::rewrite`; this module reads what the command removed from the tree, applies it
//! and hands both to the rules.

use crate::file_state::FileState;
use crate::inverse::descendants;
use meldframe_core::{ApplyError, apply_planned, plan_command};
use meldframe_protocol::{Command, Node};
use meldframe_undo::RemoteCtx;

/// What the remote command removed, read from the tree before it is applied.
fn remote_ctx(nodes: &[Node], r: &Command) -> RemoteCtx {
    let find = |id: &String| nodes.iter().find(|n| n.id == *id);
    match r {
        Command::RemoveNode { id } => RemoteCtx {
            removed: descendants(nodes, id)
                .into_iter()
                .map(String::from)
                .collect(),
            survivor: find(id).and_then(|n| n.parent_id.clone()),
            ..Default::default()
        },
        Command::Reorder { id, .. } => RemoteCtx {
            reorder_space: find(id).map(|n| (n.id.clone(), n.parent_id.clone())),
            ..Default::default()
        },
        _ => RemoteCtx::default(),
    }
}

impl FileState {
    /// Rewrite my undo and redo steps against a remote command without applying it. `before` is the
    /// tree the remote command applies to (a replica's confirmed state, not its view).
    pub fn transform_remote(&mut self, cmd: &Command, before: &[Node]) {
        let ctx = remote_ctx(before, cmd);
        self.hist.rewrite(cmd, &ctx);
    }

    /// Apply a command of another actor (already put in order by the server) without recording it, and
    /// rewrite my undo and redo steps against it. A refused command changes nothing, history included.
    pub fn apply_remote(&mut self, cmd: &Command) -> Result<(), ApplyError> {
        let pos = plan_command(&self.nodes, cmd, &self.limits)?;
        let ctx = remote_ctx(&self.nodes, cmd);
        self.hist.rewrite(cmd, &ctx);
        // A remote edit closes any keyed merge run: the step it would join may have been pruned or changed.
        self.hist.close_merge();
        apply_planned(&mut self.nodes, cmd, pos);
        self.version += 1;
        Ok(())
    }
}
