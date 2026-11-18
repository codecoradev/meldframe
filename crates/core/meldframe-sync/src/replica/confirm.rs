//! Confirming my own pending ops: by the event that names them, in or out of order, or by the answer to a send.

use super::Replica;
use crate::types::{EventOutcome, ReplicaError};
use meldframe_core::apply_command;
use meldframe_protocol::Command;
use std::collections::HashSet;

impl Replica {
    /// My pending op number `i` came back as the event `seq`. The oldest one is the normal case. A later
    /// one means the server ordered it ahead of an older op of mine (refused for now, or late): it is
    /// confirmed all the same and the ops before it are built again on the new `base`.
    pub(super) fn confirm_at(
        &mut self,
        i: usize,
        seq: u64,
        cmd: &Command,
    ) -> Result<EventOutcome, ReplicaError> {
        if i == 0 {
            self.advance_ideal(cmd);
        } else {
            self.drop_ideal();
        }
        apply_command(&mut self.base, cmd, &self.limits)
            .map_err(|e| ReplicaError::Diverged(e.to_string()))?;
        self.seq = seq;
        self.settle(i);
        Ok(match i {
            0 => EventOutcome::Confirmed,
            _ => EventOutcome::Rebased {
                dropped: self.rebuild(HashSet::new(), None, None, false, None),
            },
        })
    }

    /// Pending op number `i` is done: its history step is confirmed.
    fn settle(&mut self, i: usize) {
        let p = self.pending.remove(i).expect("the op is pending");
        if let Some(r) = p.rec {
            self.view.confirm_rec(r);
        }
        self.settle_run(p.run);
    }

    /// The server answered the send of op `key` with the number `seq` of its first write (a repeated key
    /// answers so, with nothing written). When the replica already follows the file up to `seq`, the base
    /// holds the op, whose event will not come (a snapshot was loaded past it): the op is done. Returns
    /// the number of ops that no longer applied once the rest was built again, or `None` when nothing
    /// changed (not pending, or its event is still to come).
    pub fn on_ack(&mut self, key: &str, seq: u64) -> Option<usize> {
        let i = self.pending.iter().position(|p| p.out.key == key)?;
        if seq > self.seq {
            return None;
        }
        self.drop_ideal();
        self.settle(i);
        Some(self.rebuild(HashSet::new(), None, None, false, None))
    }
}
