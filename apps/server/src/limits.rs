//! What one call may ask of the server: bounded names, bounded batches, and the cheap refusals that need no room.
//!
//! Everything here runs before a room is opened, so a refused call costs no store read.

use meldframe_host::{Op, Refusal};
use meldframe_module::RiskTier;
use meldframe_protocol::Command;
use meldframe_rooms::RegistryError;

/// Open rooms one workspace may hold at once (see `Server::with_workspace_rooms`); the global bound is
/// `MAX_OPEN_ROOMS`.
pub const MAX_ROOMS_PER_WORKSPACE: usize = 32;

/// The longest workspace or file name in bytes. With the length prefix of the room key this keeps a key
/// far under the Postgres index row limit.
pub const MAX_NAME_BYTES: usize = meldframe_text::MAX_FILE_NAME;

/// Ops in one `submit_tool` call. The same number as the MCP `batchLimit`; a larger call is refused before
/// anything is looked up (the duplicate-key check reads the store twice per op).
pub const MAX_BATCH_OPS: usize = 100;

/// Ops one workspace may send in a minute (see `Server::with_quotas`). Generous: a person editing and an agent working
/// side by side stay far under it; a runaway loop does not.
pub const MAX_OPS_PER_MINUTE: u64 = 6_000;

/// Bytes of ops one workspace may write (the JSON size of each command, see `Server::with_quotas`).
pub const MAX_STORAGE_BYTES: u64 = 256 * 1024 * 1024;

fn refused(what: &str, e: meldframe_text::TextError) -> RegistryError {
    RegistryError::InvalidName {
        reason: format!("{what} name {e}"),
    }
}

/// The one place workspace and file names are checked, by the rule of `meldframe_text` (`workspace_name`,
/// `file_name`): not empty, at most [`MAX_NAME_BYTES`], not only whitespace, no control character (NUL included) and
/// no hidden character; a file is also not `.` or `..`. The REST and WebSocket adapters check file names further (no
/// slash, `meldframe_http::valid_file`, over the same rule); this stays the floor for every caller of the server.
pub(crate) fn valid_names(workspace: &str, file: &str) -> Result<(), RegistryError> {
    meldframe_text::workspace_name(workspace).map_err(|e| refused("workspace", e))?;
    meldframe_text::file_name(file).map_err(|e| refused("file", e))
}

/// Refusals that need no room: a batch over the cap, and a Read tool with an op that is a write whatever the
/// tree (any op but a delete; a delete may be small or not, which only the tree says).
pub(crate) fn precheck(tool_tier: Option<RiskTier>, ops: &[Op]) -> Result<(), Refusal> {
    if ops.len() > MAX_BATCH_OPS {
        return Err(Refusal::BatchTooLarge {
            limit: MAX_BATCH_OPS as u64,
        });
    }
    let no_deletes = !ops
        .iter()
        .any(|o| matches!(o.command, Command::RemoveNode { .. }));
    if tool_tier == Some(RiskTier::Read) && no_deletes && !ops.is_empty() {
        return Err(Refusal::Forbidden {
            tier: RiskTier::Change,
            policy: meldframe_module::Policy::Deny,
        });
    }
    Ok(())
}
