//! `Replica`: the optimistic client model of one actor.
//!
//! - `base` is the file as the server has ordered it, up to `seq`.
//! - `pending` holds my ops that are applied locally but have not come back from the server yet.
//! - The view (`nodes()`) is always `base` plus `pending`; `check_consistency` verifies it.
//!
//! When another actor's event arrives, `base` moves on, my history is rewritten against it, and the view is
//! built again: start from `base` and apply the pending ops one by one; an op that is no longer valid is dropped.
//!
//! An undo or redo op that is already on its way when an earlier remote op arrives cannot be recalled:
//! the rebuild works out what the steps would write now and sends `UpdateNode` corrections (`correction`).
//!
//! Undo and redo are worked out by the `FileState` and sent as ordinary ops with an idempotency key.
//! One replica is one actor.

use crate::footprint::{Pairs, footprint, premise};
use crate::types::{EventOutcome, OpKind, Outgoing, ReplicaError};
use meldframe_check::LoadError;
use meldframe_core::{ApplyError, Limits, apply_command};
use meldframe_history::FileState;
use meldframe_protocol::{Command, MAX_OP_DEPS, Node};
use std::collections::{HashMap, HashSet, VecDeque};

mod rebuild;

#[derive(Debug, Clone)]
struct Pending {
    out: Outgoing,
    /// The command as text: how an event of mine is recognised when it comes back.
    json: String,
    /// The history rec of an edit (undo and redo ops have none).
    rec: Option<u64>,
    /// The undo or redo step (a run of the history) that made this op; none for an edit.
    run: Option<u64>,
    /// What this op writes, worked out when it was made.
    writes: Pairs,
    /// A correction only: what it still sets right in the model (`want`, `ideal`), without the fields that
    /// a later remote op wrote. The op already sent or queued is not changed; a later correction restores
    /// the remote value.
    intent: Option<Command>,
    /// A correction only: it was sent again after a refusal; a second refusal ends it.
    resent: bool,
    /// What this op adds to the state the rebuilt steps give (`Replica::ideal`), as of the last rebuild;
    /// none until a rebuild has worked it out (then the op itself).
    ideal_cmds: Option<Vec<Command>>,
}

impl Pending {
    /// A correction (see `correction`): an op that is no edit and belongs to no undo or redo step.
    fn is_correction(&self) -> bool {
        self.run.is_none() && self.rec.is_none() && self.out.kind != OpKind::Edit
    }
}

/// Where an op comes from in the history: the rec of an edit, or the step of an undo or redo.
#[derive(Default)]
struct Source {
    rec: Option<u64>,
    run: Option<u64>,
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
    /// Undo and redo steps: how many of their ops are still pending. At zero the step is finished.
    run_left: HashMap<u64, usize>,
    /// The confirmed state as it would be had every stale op been the op the rebuilt step works out, while
    /// that differs from `base` (a stale op that the server applied leaves its value in `base`).
    ideal: Option<Vec<Node>>,
    counter: u64,
    limits: Limits,
}

impl Replica {
    /// `actor` is the name the server puts on my events (such as `user:alice`); `key_prefix` must be
    /// unique per replica. `nodes` and `seq` come from a snapshot, which is bytes from a network and is not
    /// trusted: it goes through `FileState::try_from_nodes` (the old-file migration, then `check_file`),
    /// and a bad one is refused with a typed [`LoadError`] and nothing is repaired.
    pub fn new(
        actor: &str,
        key_prefix: &str,
        nodes: Vec<Node>,
        seq: u64,
    ) -> Result<Self, LoadError> {
        let limits = Limits::default();
        let mut view = FileState::try_from_nodes(nodes, limits.clone())?;
        view.set_optimistic(true);
        Ok(Replica {
            actor: actor.to_string(),
            prefix: key_prefix.to_string(),
            base: view.nodes().to_vec(),
            seq,
            view,
            pending: VecDeque::new(),
            outbox: Vec::new(),
            run_left: HashMap::new(),
            ideal: None,
            counter: 0,
            limits,
        })
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

    /// How many steps undo can go back.
    pub fn undo_depth(&self) -> usize {
        self.view.undo_depth()
    }

    /// How many steps redo can go forward.
    pub fn redo_depth(&self) -> usize {
        self.view.redo_depth()
    }

    /// The keys of the pending ops (oldest first, at most `MAX_OP_DEPS`, the newest kept) that an undo or
    /// redo reading `reads` was built on: the ops that wrote something it reads, and the earlier ops of
    /// the same step (`chain_from` is the index of the first pending op of that step). An edit has none:
    /// it carries the person's intent as it is.
    fn deps_for(&self, kind: OpKind, reads: &Pairs, chain_from: usize) -> Vec<String> {
        if kind == OpKind::Edit {
            return vec![];
        }
        let mut by_node: HashMap<&str, Vec<&str>> = HashMap::new();
        for (n, f) in reads {
            by_node.entry(n.as_str()).or_default().push(f.as_str());
        }
        let hit = |p: &Pending| {
            p.writes.iter().any(|(n, w)| {
                by_node
                    .get(n.as_str())
                    .is_some_and(|rs| rs.iter().any(|r| premise(r, w)))
            })
        };
        let all: Vec<String> = self
            .pending
            .iter()
            .enumerate()
            .filter(|(i, p)| *i >= chain_from || hit(p))
            .map(|(_, p)| p.out.key.clone())
            .collect();
        all[all.len().saturating_sub(MAX_OP_DEPS)..].to_vec()
    }

    /// `foot` is `(reads, writes)` of the op against the view before it was applied.
    fn enqueue(
        &mut self,
        command: Command,
        kind: OpKind,
        from: Source,
        foot: (Pairs, Pairs),
        chain_from: usize,
        forced_deps: Option<Vec<String>>,
    ) {
        self.counter += 1;
        let deps = forced_deps.unwrap_or_else(|| self.deps_for(kind, &foot.0, chain_from));
        let out = Outgoing {
            key: format!("{}-{}", self.prefix, self.counter),
            command,
            kind,
            seen_seq: self.seq,
            deps,
        };
        self.outbox.push(out.clone());
        self.pending.push_back(Pending {
            json: cmd_json(&out.command),
            out,
            rec: from.rec,
            run: from.run,
            writes: foot.1,
            intent: None,
            resent: false,
            ideal_cmds: None,
        });
    }

    /// A new local command: applied at once, recorded in the history and queued as an op.
    pub fn apply(&mut self, cmd: &Command) -> Result<(), ApplyError> {
        self.apply_keyed(cmd, None)
    }

    /// Like [`Replica::apply`]; edits with the same `key` join one history step.
    pub fn apply_keyed(&mut self, cmd: &Command, key: Option<&str>) -> Result<(), ApplyError> {
        self.apply_inner(cmd, key, None)
    }

    /// Like [`Replica::apply`], but the op names the given keys of my pending ops as its dependencies.
    /// A caller that applies an undo or redo again on a new snapshot uses it, so the op falls with its
    /// premise. A key that is no longer pending is ignored.
    pub fn apply_dependent(&mut self, cmd: &Command, deps: &[String]) -> Result<(), ApplyError> {
        let live = deps
            .iter()
            .filter(|d| self.pending.iter().any(|p| &p.out.key == *d))
            .cloned()
            .collect();
        self.apply_inner(cmd, None, Some(live))
    }

    fn apply_inner(
        &mut self,
        cmd: &Command,
        key: Option<&str>,
        forced: Option<Vec<String>>,
    ) -> Result<(), ApplyError> {
        let foot = footprint(self.view.nodes(), cmd);
        self.view.apply_keyed(cmd, key)?;
        let rec = self.view.last_rec();
        let at = self.pending.len();
        let from = Source {
            rec: Some(rec),
            run: None,
        };
        self.enqueue(cmd.clone(), OpKind::Edit, from, foot, at, forced);
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
        let before = self.view.nodes().to_vec();
        let cmds = self.view.undo();
        Ok(self.send_step(before, cmds, OpKind::Undo))
    }

    pub fn redo(&mut self) -> Result<bool, ApplyError> {
        let before = self.view.nodes().to_vec();
        let cmds = self.view.redo();
        Ok(self.send_step(before, cmds, OpKind::Redo))
    }

    /// Queue every command of one undo or redo step. The footprint of each command is worked out
    /// against the view just before it was applied (`before` is the view before the step, advanced
    /// command by command).
    fn send_step(&mut self, before: Vec<Node>, cmds: Option<Vec<Command>>, kind: OpKind) -> bool {
        let Some(cmds) = cmds else { return false };
        let chain_from = self.pending.len();
        let run = self.view.take_last_run();
        if let Some(m) = run {
            self.run_left.insert(m, cmds.len());
        }
        let mut view = before;
        for c in cmds {
            let foot = footprint(&view, &c);
            let _ = apply_command(&mut view, &c, &self.limits);
            let from = Source { rec: None, run };
            self.enqueue(c, kind, from, foot, chain_from, None);
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
            self.advance_ideal(cmd);
            apply_command(&mut self.base, cmd, &self.limits)
                .map_err(|e| ReplicaError::Diverged(e.to_string()))?;
            let p = self.pending.pop_front().expect("checked above");
            if let Some(r) = p.rec {
                self.view.confirm_rec(r);
            }
            self.settle_run(p.run);
            self.seq = seq;
            return Ok(EventOutcome::Confirmed);
        }
        let mut next = self.base.clone();
        apply_command(&mut next, cmd, &self.limits)
            .map_err(|e| ReplicaError::Diverged(e.to_string()))?;
        // Another actor (or an op of mine that was dropped): my history is rewritten against it, the
        // confirmed state moves on and the view is built again from it.
        self.view.transform_remote(cmd, &self.base);
        if let Some(w) = self.ideal.as_mut() {
            drop(apply_command(w, cmd, &self.limits));
        }
        self.base = next;
        self.seq = seq;
        Ok(EventOutcome::Rebased {
            dropped: self.rebuild(HashSet::new(), None, Some(cmd), true, None),
        })
    }

    /// One op of the step `run` is confirmed; when it was the last one, the step is finished.
    fn settle_run(&mut self, run: Option<u64>) {
        let Some(m) = run else { return };
        if let Some(n) = self.run_left.get_mut(&m) {
            *n -= 1;
            if *n == 0 {
                self.run_left.remove(&m);
                self.view.finish_run(m);
            }
        }
    }

    /// Steps whose ops fell: the history goes back, newest step first, so a step built on the result
    /// of another is put back before the step it used.
    fn put_back_runs(&mut self, mut runs: Vec<u64>, direct: Option<u64>) {
        runs.sort_unstable_by(|a, b| b.cmp(a));
        for m in runs {
            self.run_left.remove(&m);
            self.view.put_back_run(m, direct == Some(m));
        }
    }

    /// The server refused op `key`: it leaves the pending queue and its history step is forgotten, and
    /// the view is built again without its result. Returns `false` when the op is not pending (it was
    /// dropped before, or confirmed).
    pub fn on_reject(&mut self, key: &str) -> bool {
        let Some(i) = self.pending.iter().position(|p| p.out.key == key) else {
            return false;
        };
        let p = self.pending.remove(i).expect("position found");
        if let Some(r) = p.rec {
            self.view.forget_rec(r);
        }
        self.outbox.retain(|o| o.key != key);
        // A refused correction is sent once more (a second refusal ends it) so `base` keeps no stale value.
        let lost = (p.is_correction() && !p.resent).then(|| p.intent.iter().cloned().collect());
        self.rebuild(
            HashSet::from([key.to_string()]),
            p.run,
            None,
            !p.is_correction(),
            lost,
        );
        true
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
