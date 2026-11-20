//! A dry run of a batch of ops against a room: the way to make a multi-op call atomic.
//!
//! The room writes each op on its own, so a batch that fails at its third op would leave two written.
//! [`check`] replays the batch against a copy of the room's tree and against the room's log, with the rules
//! `Room::accept` applies in the same order (duplicate key, dependencies, `ifSeq`, the command itself), and
//! says which op would fail. A caller that holds the room still (the server's lock) and calls it before it
//! submits writes either all ops or none, as far as these checks go.
//!
//! A batch can also carry [`Guard`]s: an `ifSeq` for one node of a command that touches several, which the
//! room's own per-op `ifSeq` cannot express when an earlier command of the batch already touched one of them.
//! The scan of the log for changes is bounded ([`MAX_SCAN`]): an `ifSeq` further behind is refused.
//!
//! What it cannot see: the room's snapshot floor (an `ifSeq` below the snapshot the room was opened from
//! conflicts there on any node, here only on a node the log shows as changed), and a store that fails part
//! way. Both can still stop a batch after earlier ops were written; they are rare and the retry with the
//! same keys is safe.

use meldframe_core::{Limits, apply_planned_doc, plan_doc};
use meldframe_host::{Op, Refusal, Room, Store, StoreError, StoredOp};
use meldframe_protocol::{Command, Doc};
use std::collections::{HashMap, HashSet, hash_map::Entry};

/// How many log entries are read at a time when looking for changes after an `ifSeq`.
const PAGE: usize = 256;

/// The most log entries an `ifSeq` may lie behind the room's head. The check reads the log from there while
/// the caller holds the server lock, so a stale `ifSeq` (0 on a long log) is refused instead of scanned.
pub const MAX_SCAN: u64 = 2_000;

/// An `ifSeq` for one node of one command: the node must not have been touched by the log after `if_seq`.
/// Touches by earlier commands of the same batch are not counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guard {
    pub index: usize,
    pub if_seq: u64,
    pub node: String,
}

/// Why a batch must not be submitted.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    /// The op at `index` would be refused; nothing of the batch was written.
    Refused { index: usize, refusal: Refusal },
    /// The store could not be read.
    Store(StoreError),
}

impl From<StoreError> for Stop {
    fn from(e: StoreError) -> Self {
        Stop::Store(e)
    }
}

/// The ids of the nodes a command touches, as `Room::accept` counts them for `ifSeq`.
pub fn touches(cmd: &Command) -> Vec<String> {
    match cmd {
        Command::UpdateNode { id, .. }
        | Command::RemoveNode { id }
        | Command::Reorder { id, .. } => vec![id.clone()],
        Command::AddNode { node, .. } => vec![node.id.clone()],
        Command::Reparent { id, parent_id, .. } => {
            let mut v = vec![id.clone()];
            v.extend(parent_id.clone());
            v
        }
        other => meldframe_variables::touched(other),
    }
}

/// Check `ops` of `actor` against `room` without writing. `Ok` means every check `Room::accept` would make
/// passes for the whole batch.
pub fn check<S: Store>(
    room: &Room<S>,
    actor: &str,
    ops: &[Op],
    guards: &[Guard],
) -> Result<(), Stop> {
    let limits = Limits::default();
    let mut scratch = Doc {
        nodes: room.nodes().to_vec(),
        vars: room.vars().clone(),
    };
    let mut fresh_keys: HashSet<&str> = HashSet::new();
    let mut batch_touched: HashSet<String> = HashSet::new();
    // Per `ifSeq` value: the nodes the log shows as touched after it, read once.
    let mut since: HashMap<u64, HashSet<String>> = HashMap::new();
    for (index, op) in ops.iter().enumerate() {
        let refuse = |refusal: Refusal| Stop::Refused { index, refusal };
        if let Some(seq) = room.store().find(room.id(), actor, &op.key)? {
            if !same_command(room, seq, &op.command)? {
                return Err(refuse(Refusal::Invalid {
                    reason: format!("key {} was used at {seq} for a different command", op.key),
                }));
            }
            continue;
        }
        for dep in &op.deps {
            let known = fresh_keys.contains(dep.as_str())
                || room.store().find(room.id(), actor, dep)?.is_some();
            if !known {
                return Err(refuse(Refusal::DependencyRejected { key: dep.clone() }));
            }
        }
        if let Some(if_seq) = op.if_seq {
            let later = later(&mut since, room, if_seq).map_err(|e| e.at(index))?;
            for node_id in touches(&op.command) {
                if batch_touched.contains(&node_id) || later.contains(&node_id) {
                    return Err(refuse(Refusal::Conflict { node_id }));
                }
            }
        }
        for g in guards.iter().filter(|g| g.index == index) {
            if later(&mut since, room, g.if_seq)
                .map_err(|e| e.at(index))?
                .contains(&g.node)
            {
                return Err(refuse(Refusal::Conflict {
                    node_id: g.node.clone(),
                }));
            }
        }
        let pos =
            plan_doc(&scratch, &op.command, &limits).map_err(|e| refuse(Refusal::Apply(e)))?;
        apply_planned_doc(&mut scratch, &op.command, pos);
        batch_touched.extend(touches(&op.command));
        fresh_keys.insert(&op.key);
    }
    Ok(())
}

/// The newest `limit` ops after `since` that touched `node`, oldest first, out of the last [`MAX_SCAN`] ops.
pub fn changes_to<S: Store>(
    room: &Room<S>,
    since: u64,
    node: &str,
    limit: usize,
) -> Result<Vec<StoredOp>, StoreError> {
    let mut after = since.max(room.seq().saturating_sub(MAX_SCAN));
    let mut out: Vec<StoredOp> = Vec::new();
    loop {
        let page = room.store().ops_after(room.id(), after, PAGE)?;
        let Some(last) = page.last().map(|o| o.seq) else {
            break;
        };
        out.extend(
            page.into_iter()
                .filter(|o| touches(&o.command).iter().any(|id| id == node)),
        );
        if last <= after {
            break;
        }
        after = last;
    }
    out.drain(..out.len().saturating_sub(limit));
    Ok(out)
}

/// The nodes touched after `if_seq`, read once per value; refused when `if_seq` is too far behind.
fn later<'a, S: Store>(
    since: &'a mut HashMap<u64, HashSet<String>>,
    room: &Room<S>,
    if_seq: u64,
) -> Result<&'a HashSet<String>, Early> {
    let behind = room.seq().saturating_sub(if_seq);
    if behind > MAX_SCAN {
        return Err(Early::Refusal(Refusal::Invalid {
            reason: format!(
                "ifSeq {if_seq} is {behind} changes behind the file (at most {MAX_SCAN} are checked); \
                 read the file again and send its current seq"
            ),
        }));
    }
    Ok(match since.entry(if_seq) {
        Entry::Occupied(e) => e.into_mut(),
        Entry::Vacant(e) => e.insert(touched_after(room, if_seq)?),
    })
}

/// A failure of [`later`] before it knows which op it belongs to.
enum Early {
    Refusal(Refusal),
    Store(StoreError),
}

impl Early {
    fn at(self, index: usize) -> Stop {
        match self {
            Early::Refusal(refusal) => Stop::Refused { index, refusal },
            Early::Store(e) => Stop::Store(e),
        }
    }
}

impl From<StoreError> for Early {
    fn from(e: StoreError) -> Self {
        Early::Store(e)
    }
}

/// True when the log holds the same command at `seq`.
fn same_command<S: Store>(room: &Room<S>, seq: u64, command: &Command) -> Result<bool, Stop> {
    let first = room.store().ops_after(room.id(), seq - 1, 1)?.pop();
    Ok(first.is_some_and(|f| {
        f.seq == seq && serde_json::to_value(&f.command).ok() == serde_json::to_value(command).ok()
    }))
}

/// The ids of the nodes the room's log shows as touched by an op after `seq`.
fn touched_after<S: Store>(room: &Room<S>, seq: u64) -> Result<HashSet<String>, StoreError> {
    let mut out = HashSet::new();
    let mut after = seq;
    loop {
        let page = room.store().ops_after(room.id(), after, PAGE)?;
        let Some(last) = page.last().map(|o| o.seq) else {
            return Ok(out);
        };
        for op in &page {
            out.extend(touches(&op.command));
        }
        if last <= after {
            return Ok(out);
        }
        after = last;
    }
}

/// The name of a command's type, as a client sends it.
pub fn command_name(c: &Command) -> &'static str {
    match c {
        Command::AddNode { .. } => "addNode",
        Command::UpdateNode { .. } => "updateNode",
        Command::RemoveNode { .. } => "removeNode",
        Command::Reparent { .. } => "reparent",
        Command::Reorder { .. } => "reorder",
        Command::AddCollection { .. } => "addCollection",
        Command::RemoveCollection { .. } => "removeCollection",
        Command::RenameCollection { .. } => "renameCollection",
        Command::AddMode { .. } => "addMode",
        Command::RemoveMode { .. } => "removeMode",
        Command::RenameMode { .. } => "renameMode",
        Command::AddVariable { .. } => "addVariable",
        Command::RemoveVariable { .. } => "removeVariable",
        Command::RenameVariable { .. } => "renameVariable",
        Command::SetVariableValue { .. } => "setVariableValue",
    }
}
