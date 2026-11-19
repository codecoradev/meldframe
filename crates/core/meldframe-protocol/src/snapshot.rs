//! The snapshot a client starts from (`GET /v1/files/{file}`): the file as it is now, then the live stream
//! after `seq`.

use crate::node::Node;
use crate::variables::Variables;
use serde::Serialize;
use ts_rs::TS;

/// A node with who changed it last. `lastActor` and `lastSeq` come from the op log (the last op that
/// touched the node, an undo included); they are absent when the server did not look that far back. The log
/// has no time, so there is no time field.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SnapshotNode {
    #[serde(flatten)]
    #[ts(flatten)]
    pub node: Node,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub last_seq: Option<u64>,
}

/// A file at one sequence number.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileSnapshot {
    #[ts(type = "number")]
    pub seq: u64,
    pub nodes: Vec<SnapshotNode>,
    /// Collections and variables; left out when the file has none, so a file without variables reads as before.
    #[serde(skip_serializing_if = "Variables::is_empty")]
    #[ts(as = "Option<Variables>", optional)]
    pub variables: Variables,
}
