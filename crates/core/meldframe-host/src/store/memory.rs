//! The in-memory adapter of the store seam.

use super::{Appended, NewOp, Snapshot, Store, StoreError, StoredOp};
use std::collections::HashMap;

#[derive(Debug, Default)]
struct RoomLog {
    ops: Vec<StoredOp>,
    /// `(actor, key)` to sequence number.
    keys: HashMap<(String, String), u64>,
    snapshot: Option<Snapshot>,
}

/// A store that keeps everything in memory. For tests and for a room that does not need to survive a
/// restart.
#[derive(Debug, Default)]
pub struct MemoryStore {
    rooms: HashMap<String, RoomLog>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn append(&mut self, room: &str, op: NewOp) -> Result<Appended, StoreError> {
        let log = self.rooms.entry(room.to_string()).or_default();
        let id = (op.actor.clone(), op.key.clone());
        if let Some(&seq) = log.keys.get(&id) {
            let first = &log.ops[seq as usize - 1];
            if serde_json::to_value(&first.command).ok() != serde_json::to_value(&op.command).ok() {
                return Err(StoreError::KeyReused { seq });
            }
            return Ok(Appended {
                seq,
                written: false,
            });
        }
        let seq = log.ops.len() as u64 + 1;
        log.keys.insert(id, seq);
        log.ops.push(StoredOp {
            seq,
            actor: op.actor,
            key: op.key,
            command: op.command,
        });
        Ok(Appended { seq, written: true })
    }

    fn find(&self, room: &str, actor: &str, key: &str) -> Result<Option<u64>, StoreError> {
        let id = (actor.to_string(), key.to_string());
        Ok(self.rooms.get(room).and_then(|l| l.keys.get(&id).copied()))
    }

    fn latest_seq(&self, room: &str) -> Result<u64, StoreError> {
        Ok(self.rooms.get(room).map_or(0, |l| l.ops.len() as u64))
    }

    fn ops_after(&self, room: &str, seq: u64) -> Result<Vec<StoredOp>, StoreError> {
        let skip = usize::try_from(seq).unwrap_or(usize::MAX);
        Ok(self
            .rooms
            .get(room)
            .map(|l| l.ops.iter().skip(skip).cloned().collect())
            .unwrap_or_default())
    }

    fn save_snapshot(&mut self, room: &str, snapshot: Snapshot) -> Result<(), StoreError> {
        let Some(log) = self.rooms.get_mut(room) else {
            return Err(StoreError::SnapshotAhead {
                seq: snapshot.seq,
                log: 0,
            });
        };
        let head = log.ops.len() as u64;
        if snapshot.seq > head {
            return Err(StoreError::SnapshotAhead {
                seq: snapshot.seq,
                log: head,
            });
        }
        if let Some(prev) = &log.snapshot
            && snapshot.seq <= prev.seq
        {
            return Err(StoreError::SnapshotStale {
                seq: snapshot.seq,
                latest: prev.seq,
            });
        }
        log.snapshot = Some(snapshot);
        Ok(())
    }

    fn latest_snapshot(&self, room: &str) -> Result<Option<Snapshot>, StoreError> {
        Ok(self.rooms.get(room).and_then(|l| l.snapshot.clone()))
    }
}
