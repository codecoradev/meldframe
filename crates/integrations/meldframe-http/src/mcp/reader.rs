//! Read access to files for the read tools: the [`Reader`] seam and the two shapes it returns.

use crate::reply::Failure;
use meldframe_host::{Principal, StoredOp};
use meldframe_protocol::{Node, Variables};

/// A file as a read tool sees it: its sequence number and its nodes.
#[derive(Debug, Clone)]
pub struct FileView {
    pub seq: u64,
    pub nodes: Vec<Node>,
}

/// What a conflict reply says: the file's sequence number, the node now (`None` when removed) and the
/// ops that touched it after `ifSeq`, oldest first.
#[derive(Debug, Clone)]
pub struct Conflict {
    pub seq: u64,
    pub node: Option<Node>,
    pub changes: Vec<StoredOp>,
}

/// Read access to files, for the read tools. The server implements it next to its `Submitter`; the
/// implementations block, so the adapter calls them from `spawn_blocking` under the call's slot. Each method
/// is one call under one lock on one opening of the file, so what it returns is consistent.
pub trait Reader: Send + Sync {
    /// The file of the principal's workspace as it is now (a file that does not exist reads as seq 0, empty).
    fn view(&self, who: &Principal, file: &str) -> Result<FileView, Failure>;
    /// The sequence number and the variables (collections and variables) of the file as it is now.
    fn variables(&self, who: &Principal, file: &str) -> Result<(u64, Variables), Failure>;
    /// The file and its variables as one picture: the nodes, the sequence number and the variables of the file as it is now,
    /// from one read under one lock. For a tool that needs both for its own call (a copy is checked against the nodes and the
    /// variables together): it is allowed under the read policy of `get_nodes` and adds no entry to the call log, the calling
    /// tool having its own, as `head` does.
    fn view_with_variables(
        &self,
        who: &Principal,
        file: &str,
    ) -> Result<(FileView, Variables), Failure>;
    /// The file and the last `window` ops of its log (fewer when the log is shorter), in order.
    fn tree(
        &self,
        who: &Principal,
        file: &str,
        window: usize,
    ) -> Result<(FileView, Vec<StoredOp>), Failure>;
    /// The detail of a conflict on `node`: at most the newest `limit` ops after `since` that touched it,
    /// out of the last [`meldframe_batch::MAX_SCAN`] ops of the log.
    fn conflict(
        &self,
        who: &Principal,
        file: &str,
        since: u64,
        node: &str,
        limit: usize,
    ) -> Result<Conflict, Failure>;
    /// For each key, the sequence number of the principal's op with that key, if it was written.
    fn applied(
        &self,
        who: &Principal,
        file: &str,
        keys: &[String],
    ) -> Result<Vec<Option<u64>>, Failure>;
    /// The sequence number of the file, for a tool that reads it as part of its own call: not logged as a read
    /// call of its own. The default reads the whole file through [`Reader::view`].
    fn head(&self, who: &Principal, file: &str) -> Result<u64, Failure> {
        self.view(who, file).map(|v| v.seq)
    }
    /// The sequence numbers of the leading keys that are in the log, in key order, up to the first that is not: how
    /// much of a call was written. Like [`Reader::head`] it is not a call of its own (no log entry, and a deployment
    /// that denies `batch_status` still records headers). The default asks [`Reader::applied`].
    fn written(&self, who: &Principal, file: &str, keys: &[String]) -> Result<Vec<u64>, Failure> {
        Ok(self
            .applied(who, file, keys)?
            .into_iter()
            .map_while(|s| s)
            .collect())
    }
    /// Log a call that reads no file through this seam (`whoami`, a tool of another crate): `code` is `None` when
    /// it succeeded, else its stable error code. The default keeps no log.
    fn record(&self, _who: &Principal, _tool: &str, _file: &str, _code: Option<&str>) {}
}
