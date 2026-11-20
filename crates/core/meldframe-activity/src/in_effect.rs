//! Two facts about a batch that are never stored: could `revert_batch` still change anything (`inEffect`), and is the
//! whole batch in the record (`revertable`). Both are read from the log through `meldframe-revert`.

use meldframe_protocol::Node;
use meldframe_revert::{Batch, ConfirmedLog, LoggedOp, RevertError, revert_batch};

/// Whether reverting a batch would still change something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InEffect {
    /// A revert has commands to apply.
    Yes,
    /// `nothing_to_revert`: every field the batch wrote was written again since, or it was reverted.
    No,
    /// Not known: the log holds no op of the batch, or it does not replay on `before`.
    Unknown,
}

/// `before` is the tree just before the first op of `log`, and `log` runs from there to the newest op, as
/// [`revert_batch`] takes them.
pub fn in_effect(before: &[Node], log: &[LoggedOp], actor: &str, key_prefix: &str) -> InEffect {
    match revert_batch(before, log, Batch { actor, key_prefix }) {
        Ok(r) if r.commands.is_empty() => InEffect::No,
        Ok(_) => InEffect::Yes,
        Err(RevertError::NoSuchBatch | RevertError::LogDoesNotReplay { .. }) => InEffect::Unknown,
    }
}

/// Whether the record holds every op of the batch: some op of it is there, none was folded into the base, and the
/// record still replays.
pub fn revertable(record: &ConfirmedLog, actor: &str, key_prefix: &str) -> bool {
    !record.diverged()
        && !record.batch_is_partial(actor, key_prefix)
        && record.ops().iter().any(|o| {
            o.actor == actor
                && meldframe_revert::batch_index(&o.key).is_some_and(|(p, _)| p == key_prefix)
        })
}

/// The answer for the selected row, kept while the confirmed `seq` and the batch stay the same.
#[derive(Debug, Default)]
pub struct InEffectMemo(Option<(u64, String, String, InEffect)>);

impl InEffectMemo {
    pub fn get(
        &mut self,
        confirmed_seq: u64,
        actor: &str,
        key_prefix: &str,
        compute: impl FnOnce() -> InEffect,
    ) -> InEffect {
        if let Some((s, a, p, v)) = &self.0
            && (*s, a.as_str(), p.as_str()) == (confirmed_seq, actor, key_prefix)
        {
            return *v;
        }
        let v = compute();
        self.0 = Some((confirmed_seq, actor.to_owned(), key_prefix.to_owned(), v));
        v
    }
}
