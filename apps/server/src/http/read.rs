//! The reads of the MCP surface: the file views of the [`Reader`], each under the policy of its tool and in the call log.

use super::SharedServer;
use crate::AnyStore;
use meldframe_host::{Principal, Room, Store, StoreError, StoredOp};
use meldframe_http::Failure;
use meldframe_http::mcp::{Conflict, FileView, Reader};
use meldframe_module::Policy;

impl SharedServer {
    /// Run `f` on the room of `file`, opening it for the call when it is closed. A read tool is allowed
    /// only when the registry has it with an `Auto` policy; without the MCP module nothing reads.
    fn read_unlogged<R>(
        &self,
        tool: &str,
        who: &Principal,
        file: &str,
        f: impl FnOnce(&Room<AnyStore>) -> R,
    ) -> Result<R, Failure> {
        if self.lock().registry().policy_for(tool) != Some(Policy::Auto) {
            return Err(Failure::Refused(meldframe_host::Refusal::Forbidden {
                tier: meldframe_module::RiskTier::Read,
                policy: Policy::Deny,
            }));
        }
        self.with_open_room(who, file, f)
    }

    /// `read_unlogged`, and one entry in the call log for the call.
    fn read<R>(
        &self,
        tool: &str,
        who: &Principal,
        file: &str,
        f: impl FnOnce(&Room<AnyStore>) -> R,
    ) -> Result<R, Failure> {
        let done = self.read_unlogged(tool, who, file, f);
        self.log_read(tool, who, file, &done);
        done
    }

    /// `read` for a read whose own query can fail in the store: that failure is the outcome of the call, so
    /// the log says `unavailable`, as the caller hears, and not `ok`.
    fn read_try<T>(
        &self,
        tool: &str,
        who: &Principal,
        file: &str,
        f: impl FnOnce(&Room<AnyStore>) -> Result<T, StoreError>,
    ) -> Result<T, Failure> {
        let done = self
            .read_unlogged(tool, who, file, f)
            .and_then(|r| r.map_err(unavailable));
        self.log_read(tool, who, file, &done);
        done
    }

    fn log_read<R>(&self, tool: &str, who: &Principal, file: &str, done: &Result<R, Failure>) {
        let refusal = match done {
            Err(Failure::Refused(r)) => Some(r),
            _ => None,
        };
        let outcome = done.as_ref().map(|_| ()).map_err(|_| refusal);
        self.lock().record_call_as(tool, file, who, outcome);
    }
}

fn unavailable(e: StoreError) -> Failure {
    Failure::Unavailable(e.to_string())
}

/// The file for `export_frame`: the nodes and the variables under one lock. Not logged here: the MCP handler writes
/// the one entry of the call.
#[cfg(feature = "export")]
impl meldframe_http_export::FrameSource for SharedServer {
    fn read(&self, who: &Principal, file: &str) -> Result<meldframe_http_export::Frames, Failure> {
        self.read_unlogged(export::TOOL, who, file, |r| meldframe_http_export::Frames {
            seq: r.seq(),
            nodes: r.nodes().to_vec(),
            vars: r.vars().clone(),
        })
    }
}

impl Reader for SharedServer {
    fn record(&self, who: &Principal, tool: &str, file: &str, code: Option<&str>) {
        self.lock().record_call_code(tool, file, who, code);
    }

    fn head(&self, who: &Principal, file: &str) -> Result<u64, Failure> {
        // Allowed under the same read policy as `get_nodes`; the calling tool has its own log entry.
        self.read_unlogged("get_nodes", who, file, |r| r.seq())
    }

    fn view(&self, who: &Principal, file: &str) -> Result<FileView, Failure> {
        self.read("get_nodes", who, file, |r| FileView {
            seq: r.seq(),
            nodes: r.nodes().to_vec(),
        })
    }

    fn variables(
        &self,
        who: &Principal,
        file: &str,
    ) -> Result<(u64, meldframe_protocol::Variables), Failure> {
        self.read("get_variables", who, file, |r| (r.seq(), r.vars().clone()))
    }

    fn view_with_variables(
        &self,
        who: &Principal,
        file: &str,
    ) -> Result<(FileView, meldframe_protocol::Variables), Failure> {
        // One room, one lock: the nodes, the variables and the seq are one picture. Under the policy of `get_nodes`, and
        // not logged: the calling tool has its own entry.
        self.read_unlogged("get_nodes", who, file, |r| {
            (
                FileView {
                    seq: r.seq(),
                    nodes: r.nodes().to_vec(),
                },
                r.vars().clone(),
            )
        })
    }

    fn tree(
        &self,
        who: &Principal,
        file: &str,
        window: usize,
    ) -> Result<(FileView, Vec<StoredOp>), Failure> {
        self.read_try("get_tree", who, file, |r| {
            let from = r.seq().saturating_sub(window as u64);
            let recent = r.store().ops_after(r.id(), from, window)?;
            Ok((
                FileView {
                    seq: r.seq(),
                    nodes: r.nodes().to_vec(),
                },
                recent,
            ))
        })
    }

    fn conflict(
        &self,
        who: &Principal,
        file: &str,
        since: u64,
        node: &str,
        limit: usize,
    ) -> Result<Conflict, Failure> {
        // Part of the `apply_commands` call that conflicted: that call has its own entry.
        self.read_unlogged("get_tree", who, file, |r| {
            let changes = meldframe_batch::changes_to(r, since, node, limit)?;
            Ok(Conflict {
                seq: r.seq(),
                node: r.nodes().iter().find(|n| n.id == node).cloned(),
                changes,
            })
        })?
        .map_err(unavailable)
    }

    fn written(&self, who: &Principal, file: &str, keys: &[String]) -> Result<Vec<u64>, Failure> {
        // Under the policy of `get_nodes` and not logged: the calling tool has its own entry.
        self.read_unlogged("get_nodes", who, file, |r| {
            let find = |k| r.store().find(r.id(), &who.actor, k).transpose();
            keys.iter()
                .map_while(|k| find(k))
                .collect::<Result<Vec<_>, _>>()
        })?
        .map_err(unavailable)
    }

    fn applied(
        &self,
        who: &Principal,
        file: &str,
        keys: &[String],
    ) -> Result<Vec<Option<u64>>, Failure> {
        self.read_try("batch_status", who, file, |r| {
            keys.iter()
                .map(|k| r.store().find(r.id(), &who.actor, k))
                .collect::<Result<Vec<_>, _>>()
        })
    }
}
