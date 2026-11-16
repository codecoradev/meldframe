//! What one call may ask of the server: bounded names, bounded batches, and the cheap refusals that need no
//! room. Everything here runs before a room is opened, so a refused call costs no store read.

use meldframe_host::{Op, Refusal};
use meldframe_module::RiskTier;
use meldframe_protocol::Command;
use meldframe_rooms::RegistryError;

/// Open rooms one workspace may hold at once (see `Server::with_workspace_rooms`); the global bound is
/// `MAX_OPEN_ROOMS`.
pub const MAX_ROOMS_PER_WORKSPACE: usize = 32;

/// The longest workspace or file name in bytes. With the length prefix of the room key this keeps a key
/// far under the Postgres index row limit.
pub const MAX_NAME_BYTES: usize = 128;

/// Ops in one `submit_tool` call. The same number as the MCP `batchLimit`; a larger call is refused before
/// anything is looked up (the duplicate-key check reads the store twice per op).
pub const MAX_BATCH_OPS: usize = 100;

/// Characters that change how a name reads without showing: bidi controls, zero-width marks, line and
/// paragraph separators, the byte order mark.
fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2028}' | '\u{2029}'
        | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

fn check(what: &str, name: &str, dots: bool) -> Result<(), RegistryError> {
    let reason = if name.is_empty() {
        "is empty"
    } else if name.len() > MAX_NAME_BYTES {
        "is longer than 128 bytes"
    } else if name.trim().is_empty() {
        "is only whitespace"
    } else if dots && (name == "." || name == "..") {
        "is a dot name"
    } else if name.chars().any(|c| c.is_control() || is_invisible(c)) {
        "has a control or invisible formatting character"
    } else {
        return Ok(());
    };
    Err(RegistryError::InvalidName {
        reason: format!("{what} name {reason}"),
    })
}

/// The one place workspace and file names are checked, by one rule: not empty, at most [`MAX_NAME_BYTES`],
/// not only whitespace, no control character (NUL included) and no invisible formatting character; a file
/// is also not `.` or `..`. The REST and WebSocket adapters check file names further (no slash); this stays
/// the floor for every caller of the server.
pub(crate) fn valid_names(workspace: &str, file: &str) -> Result<(), RegistryError> {
    check("workspace", workspace, false)?;
    check("file", file, true)
}

/// Refusals that need no room: a batch over the cap, and a Read tool with an op that is a write whatever the
/// tree (any op but a delete; a delete may be small or not, which only the tree says).
pub(crate) fn precheck(tool_tier: Option<RiskTier>, ops: &[Op]) -> Result<(), Refusal> {
    if ops.len() > MAX_BATCH_OPS {
        return Err(Refusal::Invalid {
            reason: format!(
                "a call takes at most {MAX_BATCH_OPS} ops; this call has {}",
                ops.len()
            ),
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
