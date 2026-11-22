//! Review (E5.2, E5.3), the parts that read no `App`: the record of the ops the server confirmed since the file was
//! loaded, and the plans of a revert and of a restore, worked out with the rules of `meldframe-revert`. The checkpoint
//! list is in `checkpoints.rs`.

use meldframe_app_model::{RevertSkip, RevertSkipReason};
use meldframe_protocol::{Command, Node};
use meldframe_revert::{
    Batch, ConfirmedLog, LoggedOp, RevertError, SkipReason, restore_commands, revert_batch,
    state_at,
};
use meldframe_sync::Outgoing;

/// The confirmed ops since the file was loaded (a bounded record), which a revert is worked out from.
#[derive(Debug)]
pub struct Review {
    pub log: ConfirmedLog,
}

impl Review {
    pub fn new(base: Vec<Node>, base_seq: u64) -> Self {
        Review {
            log: ConfirmedLog::new(base, base_seq),
        }
    }

    /// Start again from a loaded file: the ops kept so far no longer lead to it.
    pub fn reset(&mut self, base: Vec<Node>, base_seq: u64) {
        self.log.reset(base, base_seq);
    }
}

/// The confirmed ops, then the person's own unconfirmed ones as if the server numbered them next.
pub fn full_log(
    log: &ConfirmedLog,
    me: &str,
    confirmed_seq: u64,
    unconfirmed: Vec<Outgoing>,
) -> Vec<LoggedOp> {
    let mut ops = log.ops().to_vec();
    let next = confirmed_seq + 1;
    for (i, o) in unconfirmed.into_iter().enumerate() {
        ops.push(LoggedOp {
            seq: next + i as u64,
            actor: me.to_string(),
            key: o.key,
            command: o.command,
        });
    }
    ops
}

/// The commands that revert a batch, and what had to be left alone.
#[derive(Debug)]
pub struct Revert {
    pub commands: Vec<Command>,
    pub skipped: Vec<RevertSkip>,
}

/// Work out the revert of a batch, or the code of the refusal. `full` is asked for only when the record is whole.
pub fn plan_revert(
    log: &ConfirmedLog,
    actor: &str,
    key_prefix: &str,
    full: impl FnOnce() -> Vec<LoggedOp>,
) -> Result<Revert, &'static str> {
    let batch = Batch { actor, key_prefix };
    if log.diverged() {
        return Err("log_mismatch");
    }
    if log.batch_is_partial(actor, key_prefix) {
        return Err("batch_incomplete");
    }
    let r = match revert_batch(log.base(), &full(), batch) {
        Ok(r) => r,
        Err(RevertError::NoSuchBatch) => return Err("unknown_batch"),
        Err(RevertError::LogDoesNotReplay { .. }) => return Err("log_mismatch"),
    };
    if r.commands.is_empty() {
        return Err("nothing_to_revert");
    }
    let skipped = r
        .skipped
        .into_iter()
        .map(|s| RevertSkip {
            node: s.node,
            what: s.what,
            reason: match s.reason {
                SkipReason::ChangedSince => RevertSkipReason::ChangedSince,
                SkipReason::NotApplicable => RevertSkipReason::NotApplicable,
            },
        })
        .collect();
    Ok(Revert {
        commands: r.commands,
        skipped,
    })
}

/// Work out the commands that bring `current` back to the file as it was at `seq`, from the record, or the code of the
/// refusal.
pub fn plan_restore(
    log: &ConfirmedLog,
    seq: u64,
    current: &[Node],
) -> Result<Vec<Command>, &'static str> {
    // A checkpoint older than the record cannot be worked out from it.
    if seq < log.base_seq() {
        return Err("not_in_log");
    }
    if log.diverged() {
        return Err("log_mismatch");
    }
    let target = state_at(log.base(), log.ops(), seq);
    let cmds = target.ok().and_then(|t| restore_commands(&t, current).ok());
    let Some(cmds) = cmds else {
        return Err("log_mismatch");
    };
    if cmds.is_empty() {
        return Err("nothing_to_restore");
    }
    Ok(cmds)
}
