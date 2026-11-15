//! The room: one total order per file, and the only write entry, [`Room::accept`].

use crate::{Admission, Appended, NewOp, Refusal, Snapshot, Store, StoreError};
use meldframe_core::{Limits, apply_command, apply_planned, plan_command};
use meldframe_protocol::{Command, Node};
use std::collections::HashMap;
use std::fmt;

/// Take a snapshot after this many ops (unless `with_snapshot_every` says otherwise).
const SNAPSHOT_EVERY: u64 = 500;

/// How many ops `Room::open` asks the store for at a time when it replays the log.
pub const REPLAY_PAGE: usize = 256;

/// What `accept` did with an op that was not refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accepted {
    pub seq: u64,
    /// False when the key was already applied: `seq` is the first write's and nothing was written.
    pub written: bool,
}

/// A snapshot that could not be saved after its op was written. The op is accepted regardless; this tells
/// the caller (a server logs it) that a restart will replay the log from the older snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotWarning {
    /// The sequence number the snapshot was for.
    pub seq: u64,
    /// The store's error for that save.
    pub error: StoreError,
}

/// Why `accept` or `open` did not succeed.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomError {
    /// The op was refused; nothing was written.
    Refused(Refusal),
    /// The store failed before the op was written, so it was not applied. (A snapshot that fails to save
    /// after the op was written is not an error: see [`Room::snapshot_warning`].)
    Store(StoreError),
    /// The log does not replay: op `seq` was refused by the document core. The log is never edited,
    /// so this needs a person.
    Corrupt { seq: u64 },
}

impl fmt::Display for RoomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoomError::Refused(r) => write!(f, "{r}"),
            RoomError::Store(e) => write!(f, "{e}"),
            RoomError::Corrupt { seq } => write!(f, "the log does not replay at op {seq}"),
        }
    }
}

impl std::error::Error for RoomError {}

impl From<Refusal> for RoomError {
    fn from(r: Refusal) -> Self {
        RoomError::Refused(r)
    }
}

impl From<StoreError> for RoomError {
    fn from(e: StoreError) -> Self {
        RoomError::Store(e)
    }
}

/// One room per file. It owns the file's tree and its store, so `&mut self` gives one total order:
/// the order of `accept` calls is the order of sequence numbers.
#[derive(Debug)]
pub struct Room<S: Store> {
    workspace: String,
    id: String,
    store: S,
    nodes: Vec<Node>,
    limits: Limits,
    /// Node id to the sequence number of the last op that touched it. A node that is not here was
    /// last touched at `floor` or before.
    touched: HashMap<String, u64>,
    /// The snapshot's sequence number after a restart, 0 otherwise.
    floor: u64,
    seq: u64,
    snapshot_every: u64,
    snapshot_failures: u64,
    snapshot_warning: Option<SnapshotWarning>,
}

impl<S: Store> Room<S> {
    /// Open the room of file `id` in `workspace`: load the latest snapshot and replay the log after it.
    pub fn open(
        workspace: impl Into<String>,
        id: impl Into<String>,
        store: S,
    ) -> Result<Self, RoomError> {
        let id = id.into();
        let limits = Limits::default();
        let (mut nodes, floor) = match store.latest_snapshot(&id)? {
            Some(s) => (s.nodes, s.seq),
            None => (Vec::new(), 0),
        };
        let mut touched = HashMap::new();
        let mut after = floor;
        loop {
            let page = store.ops_after(&id, after, REPLAY_PAGE)?;
            let Some(last) = page.last().map(|op| op.seq) else {
                break;
            };
            // A store that ignores `seq` would return the same page for ever: a hang becomes an error.
            if last <= after {
                return Err(RoomError::Corrupt { seq: last });
            }
            for op in &page {
                apply_command(&mut nodes, &op.command, &limits)
                    .map_err(|_| RoomError::Corrupt { seq: op.seq })?;
                for node in touches(&op.command) {
                    touched.insert(node, op.seq);
                }
            }
            after = last;
        }
        let seq = store.latest_seq(&id)?;
        Ok(Room {
            workspace: workspace.into(),
            id,
            store,
            nodes,
            limits,
            touched,
            floor,
            seq,
            snapshot_every: SNAPSHOT_EVERY,
            snapshot_failures: 0,
            snapshot_warning: None,
        })
    }

    /// Save a snapshot after every `n` ops (at least 1).
    pub fn with_snapshot_every(mut self, n: u64) -> Self {
        self.snapshot_every = n.max(1);
        self
    }

    /// The tree after the last accepted op.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The file id this room serves.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The workspace the file belongs to.
    pub fn workspace(&self) -> &str {
        &self.workspace
    }

    /// The sequence number of the last accepted op, 0 for a new room.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// The newest snapshot that could not be saved, with the store's error, or `None` when the last
    /// attempt succeeded (or none was made). A snapshot only shortens the next restart, so a failed save
    /// never fails an op that is already in the log; the next multiple of the interval tries again. The
    /// warning stays set across ops that attempt no save and is cleared only by a save that works.
    pub fn snapshot_warning(&self) -> Option<&SnapshotWarning> {
        self.snapshot_warning.as_ref()
    }

    /// How many snapshots could not be saved in total (never reset, for a metric).
    pub fn snapshot_failures(&self) -> u64 {
        self.snapshot_failures
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// Give the store back, for example to open the room again.
    pub fn into_store(self) -> S {
        self.store
    }

    /// Accept an admitted op. This is the only way to write to a room, and an [`Admission`] only comes from
    /// the gate with its op and permit sealed together (see the crate docs for the compile-fail proof).
    ///
    /// Order of checks: the permit is for this room's workspace; a key
    /// that was applied before is a duplicate (same number, nothing written) when the command is the same
    /// and `invalid` when it is not; every dependency was applied for this actor (applied
    /// keys are never forgotten, they live in the log); `if_seq`; the command itself. A refused op writes
    /// nothing, and its key stays free. `if_seq` compares against the last op that touched the node; after a
    /// restart from a snapshot a node with no later op counts as touched at the snapshot, so an `if_seq`
    /// below the snapshot conflicts (safe, never lets a stale edit through). Adding a child does not count as
    /// touching its parent. A snapshot that fails to save after the write does not fail the op: see
    /// [`Room::snapshot_warning`].
    pub fn accept(&mut self, admission: Admission) -> Result<Accepted, RoomError> {
        let (op, permit) = admission.into_parts();
        // An admission cannot pair an op with another's permit; this holds for every admission.
        debug_assert_eq!(
            permit.key(),
            op.key,
            "an admission pairs an op with another op's permit"
        );
        debug_assert_eq!(
            permit.tier(),
            op.tier(),
            "an admission pairs an op with a permit of another tier"
        );
        if permit.workspace() != self.workspace {
            return Err(Refusal::Invalid {
                reason: format!(
                    "the permit is for workspace {}, not {}",
                    permit.workspace(),
                    self.workspace
                ),
            }
            .into());
        }
        let actor = permit.actor();
        if self.store.find(&self.id, actor, &op.key)?.is_some() {
            // The store decides: the same command is a duplicate, another command is a reused key.
            return match self.store.append(
                &self.id,
                NewOp {
                    actor: actor.to_string(),
                    key: op.key.clone(),
                    command: op.command.clone(),
                },
            ) {
                Ok(Appended { seq, written }) => Ok(Accepted { seq, written }),
                Err(StoreError::KeyReused { seq }) => Err(Refusal::Invalid {
                    reason: format!("key {} was used at {seq} for a different command", op.key),
                }
                .into()),
                Err(e) => Err(e.into()),
            };
        }
        for dep in &op.deps {
            if self.store.find(&self.id, actor, dep)?.is_none() {
                return Err(Refusal::DependencyRejected { key: dep.clone() }.into());
            }
        }
        if let Some(if_seq) = op.if_seq {
            for node_id in touches(&op.command) {
                if self.touched.get(&node_id).copied().unwrap_or(self.floor) > if_seq {
                    return Err(Refusal::Conflict { node_id }.into());
                }
            }
        }
        let pos = plan_command(&self.nodes, &op.command, &self.limits).map_err(Refusal::Apply)?;
        let Appended { seq, written } = self.store.append(
            &self.id,
            NewOp {
                actor: actor.to_string(),
                key: op.key.clone(),
                command: op.command.clone(),
            },
        )?;
        apply_planned(&mut self.nodes, &op.command, pos);
        self.seq = seq;
        for node in touches(&op.command) {
            self.touched.insert(node, seq);
        }
        if seq.is_multiple_of(self.snapshot_every) {
            let snapshot = Snapshot {
                seq,
                nodes: self.nodes.clone(),
            };
            self.snapshot_warning = match self.store.save_snapshot(&self.id, snapshot) {
                Ok(()) => None,
                Err(error) => {
                    self.snapshot_failures += 1;
                    Some(SnapshotWarning { seq, error })
                }
            };
        }
        Ok(Accepted { seq, written })
    }
}

/// The ids of the nodes a command touches, for `if_seq`.
fn touches(cmd: &Command) -> Vec<String> {
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
    }
}
