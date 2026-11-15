//! Accept an op in a room and record it in the activity log.

use crate::{ActivityEntry, ActivityError, ActivityLog};
use meldframe_host::{Accepted, Op, Permit, Room, RoomError, Store};

/// What `accept_logged` did with an op that was not refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logged {
    pub accepted: Accepted,
    /// Set when the entry could not be written. The op is already in the store, so it stays accepted; the
    /// caller counts or reports the failure. The log is best effort: the missing entry shows as a gap in
    /// `seq`, and a retry of the same key does not write it (see the `activity` module doc).
    pub log_error: Option<ActivityError>,
}

/// `Room::accept`, then one activity entry when the op was written. A retried key (`written: false`) and a
/// refused op record nothing. `at` is the caller's clock value; this crate reads no wall clock.
pub fn accept_logged<S: Store, L: ActivityLog + ?Sized>(
    room: &mut Room<S>,
    log: &mut L,
    op: Op,
    permit: Permit,
    at: u64,
) -> Result<Logged, RoomError> {
    let tier = permit.tier();
    let actor = permit.actor().to_string();
    let (kind, node_id) = ActivityEntry::describe(&op.command);
    let accepted = room.accept(op, permit)?;
    let log_error = if accepted.written {
        let entry = ActivityEntry {
            seq: accepted.seq,
            actor,
            kind,
            node_id,
            at,
            tier,
        };
        log.record(room.id(), entry).err()
    } else {
        None
    };
    Ok(Logged {
        accepted,
        log_error,
    })
}
