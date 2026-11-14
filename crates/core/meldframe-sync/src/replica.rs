//! `Replica`: the optimistic client model of one actor.
//!
//! - `base` is the file as the server has ordered it, up to `seq`.
//! - `pending` holds my ops that are applied locally but have not come back from the server yet.
//! - The view (`nodes()`) is always `base` plus `pending`; `check_consistency` verifies it.
//!
//! When another actor's event arrives, `base` moves on, my history is rewritten against it, and the view is
//! built again: start from `base` and apply the pending ops one by one; an op that is no longer valid is dropped.
//!
//! Undo and redo are worked out by the `FileState` and sent as ordinary ops with an idempotency key.
//! One replica is one actor.

use crate::types::{EventOutcome, OpKind, Outgoing, ReplicaError};
use meldframe_core::{ApplyError, Limits, apply_command};
use meldframe_history::FileState;
use meldframe_protocol::{Command, Node};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
struct Pending {
    out: Outgoing,
    /// The command as text: how an event of mine is recognised when it comes back.
    json: String,
    /// The history rec of an edit (undo and redo ops have none).
    rec: Option<u64>,
}

fn cmd_json(c: &Command) -> String {
    serde_json::to_string(c).unwrap_or_default()
}

/// The optimistic layer over a [`FileState`].
#[derive(Debug)]
pub struct Replica {
    actor: String,
    prefix: String,
    base: Vec<Node>,
    seq: u64,
    view: FileState,
    pending: VecDeque<Pending>,
    outbox: Vec<Outgoing>,
    counter: u64,
    limits: Limits,
}

impl Replica {
    /// `actor` is the name the server puts on my events (such as `user:alice`); `key_prefix` must be
    /// unique per replica. `nodes` and `seq` come from a snapshot.
    pub fn new(actor: &str, key_prefix: &str, nodes: Vec<Node>, seq: u64) -> Self {
        let mut view = FileState::from_nodes(nodes);
        view.set_optimistic(true);
        Replica {
            actor: actor.to_string(),
            prefix: key_prefix.to_string(),
            base: view.nodes().to_vec(),
            seq,
            view,
            pending: VecDeque::new(),
            outbox: Vec::new(),
            counter: 0,
            limits: Limits::default(),
        }
    }

    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// The view: the confirmed state plus my pending ops.
    pub fn nodes(&self) -> &[Node] {
        self.view.nodes()
    }

    /// The confirmed state.
    pub fn base(&self) -> &[Node] {
        &self.base
    }

    /// The last server sequence number applied.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn can_undo(&self) -> bool {
        self.view.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.view.can_redo()
    }

    fn enqueue(&mut self, command: Command, kind: OpKind, rec: Option<u64>) {
        self.counter += 1;
        let out = Outgoing {
            key: format!("{}-{}", self.prefix, self.counter),
            command,
            kind,
            seen_seq: self.seq,
        };
        self.outbox.push(out.clone());
        self.pending.push_back(Pending {
            json: cmd_json(&out.command),
            out,
            rec,
        });
    }

    /// A new local command: applied at once, recorded in the history and queued as an op.
    pub fn apply(&mut self, cmd: &Command) -> Result<(), ApplyError> {
        self.apply_keyed(cmd, None)
    }

    /// Like [`Replica::apply`]; edits with the same `key` join one history step.
    pub fn apply_keyed(&mut self, cmd: &Command, key: Option<&str>) -> Result<(), ApplyError> {
        self.view.apply_keyed(cmd, key)?;
        let rec = self.view.last_rec();
        self.enqueue(cmd.clone(), OpKind::Edit, Some(rec));
        Ok(())
    }

    /// Start one history step that covers many commands.
    pub fn begin_group(&mut self) {
        self.view.begin_group();
    }

    pub fn end_group(&mut self) {
        self.view.end_group();
    }

    /// Undo my last step; each command that was applied is sent as an op. `false` when there is
    /// nothing to undo.
    pub fn undo(&mut self) -> Result<bool, ApplyError> {
        let cmds = self.view.undo();
        Ok(self.send_step(cmds, OpKind::Undo))
    }

    pub fn redo(&mut self) -> Result<bool, ApplyError> {
        let cmds = self.view.redo();
        Ok(self.send_step(cmds, OpKind::Redo))
    }

    fn send_step(&mut self, cmds: Option<Vec<Command>>, kind: OpKind) -> bool {
        let Some(cmds) = cmds else { return false };
        for c in cmds {
            self.enqueue(c, kind, None);
        }
        true
    }

    /// The ops the transport has not taken yet.
    pub fn take_outbox(&mut self) -> Vec<Outgoing> {
        std::mem::take(&mut self.outbox)
    }

    /// Every op the server has not confirmed, to send again after a dropped connection (the same key
    /// makes a repeat harmless).
    pub fn unconfirmed(&self) -> Vec<Outgoing> {
        self.pending.iter().map(|p| p.out.clone()).collect()
    }

    /// Apply a server event. Events must arrive in `seq` order.
    pub fn on_event(
        &mut self,
        seq: u64,
        actor: &str,
        cmd: &Command,
    ) -> Result<EventOutcome, ReplicaError> {
        if seq <= self.seq {
            return Ok(EventOutcome::Duplicate);
        }
        if seq != self.seq + 1 {
            return Err(ReplicaError::Gap {
                expected: self.seq + 1,
                got: seq,
            });
        }
        let own = actor == self.actor
            && self
                .pending
                .front()
                .is_some_and(|p| p.json == cmd_json(cmd));
        if own {
            apply_command(&mut self.base, cmd, &self.limits)
                .map_err(|e| ReplicaError::Diverged(e.to_string()))?;
            let p = self.pending.pop_front().expect("checked above");
            if let Some(r) = p.rec {
                self.view.confirm_rec(r);
            }
            self.seq = seq;
            return Ok(EventOutcome::Confirmed);
        }
        let mut next = self.base.clone();
        apply_command(&mut next, cmd, &self.limits)
            .map_err(|e| ReplicaError::Diverged(e.to_string()))?;
        // Another actor (or an op of mine that was dropped): my history is rewritten against it, the
        // confirmed state moves on and the view is built again from it.
        self.view.transform_remote(cmd, &self.base);
        self.base = next;
        self.seq = seq;
        Ok(EventOutcome::Rebased {
            dropped: self.rebuild(),
        })
    }

    /// Build the view again: start from `base` and apply the pending ops one by one. An op that no
    /// longer applies is dropped, with its history. Returns how many were dropped.
    fn rebuild(&mut self) -> usize {
        self.view.reset_nodes(self.base.clone());
        let mut dropped = 0;
        for p in std::mem::take(&mut self.pending) {
            if self.view.replay_pending(&p.out.command, p.rec).is_ok() {
                self.pending.push_back(p);
                continue;
            }
            if let Some(r) = p.rec {
                self.view.forget_rec(r);
            }
            self.outbox.retain(|o| o.key != p.out.key);
            dropped += 1;
        }
        dropped
    }

    /// The internal invariant: the view equals `base` plus `pending`, and every pending op is valid.
    pub fn check_consistency(&self) -> Result<(), String> {
        let mut n = self.base.clone();
        for p in &self.pending {
            apply_command(&mut n, &p.out.command, &self.limits)
                .map_err(|e| format!("pending {} is not valid: {e}", p.out.key))?;
        }
        let a = serde_json::to_string(&n).unwrap_or_default();
        let b = serde_json::to_string(self.view.nodes()).unwrap_or_default();
        if a != b {
            return Err("view != base + pending".into());
        }
        Ok(())
    }
}
