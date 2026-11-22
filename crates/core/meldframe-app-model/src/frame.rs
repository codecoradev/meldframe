//! The messages of the live connection, as the transport decodes them.

use crate::activity::BatchHeader;
use crate::checkpoint::CheckpointRow;
use meldframe_protocol::Command;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A message of the live connection, as the transport decodes it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[allow(
    clippy::large_enum_variant,
    reason = "short-lived values; boxing would leak into every consumer pattern"
)]
pub enum Frame {
    /// Nothing to say; the connection is alive.
    Heartbeat {},
    /// The server dropped frames after `after_seq` for this client (it was too slow): subscribe again from
    /// there. `latest_seq` is the newest number the server has.
    Resync {
        #[ts(type = "number")]
        after_seq: u64,
        #[ts(type = "number")]
        latest_seq: u64,
    },
    /// An accepted op, numbered, with the actor the server stamped on it. The person's own ops come back
    /// this way, which confirms them. `key` is the idempotency key of the op: the app recognises its own op
    /// by it. A shell that does not carry it (an old server) leaves it out, and the app compares the actor
    /// and the command instead.
    Event {
        #[ts(type = "number")]
        seq: u64,
        actor: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        command: Command,
    },
    /// The server answered the send of the op with this key: it was applied as event `seq` (for a repeated
    /// key, the first time, and nothing was written). When the app already follows the file up to `seq`
    /// (a reload went past it) the op is done; otherwise its event is still to come and this changes
    /// nothing. Contract of the shell: after a reload it sends the unconfirmed ops again and must report
    /// every answer this way, including the one for a repeated key; until it does, an op the snapshot already
    /// holds stays pending and counts against the cap. The shell side is not built yet: only the test harness
    /// of `meldframe-app-tests` emits this frame.
    Acked {
        key: String,
        #[ts(type = "number")]
        seq: u64,
    },
    /// The server refused the op with this idempotency key. No wording: the front end writes the text.
    Rejected {
        key: String,
        code: String,
        retryable: bool,
    },
    /// The header of a numbered batch was written or changed (`ServerMsg::Batch`, ephemeral: a hint over the log, so a
    /// lost one is healed by a fetch). `unreviewed_count` is the server's count as of this change.
    Batch {
        header: BatchHeader,
        unreviewed_count: u32,
    },
    /// An actor changed these nodes (`ServerMsg::Touch`, ephemeral): the app draws its touch ring unless the actor is
    /// the person. `seq` is the newest op it is about.
    Touch {
        actor: String,
        ids: Vec<String>,
        #[ts(type = "number")]
        seq: u64,
    },
    /// A checkpoint row was made (`ServerMsg::Checkpoint`, ephemeral: a hint, the list route is the truth). A row more
    /// than one `n` above the highest held makes the app fetch the list.
    Checkpoint { checkpoint: CheckpointRow },
}
