//! The server behind the HTTP adapter: [`SharedServer`] is the one [`Submitter`], so REST and WebSocket reach
//! a room only through `Server::submit_tool` (the gate).

use crate::{
    AnyStore, PERMANENT_STORE_ERROR, Server, StreamId, SubmitError, SubscribeError, Subscription,
};
use meldframe_batch::Stop;
use meldframe_host::{Accepted, Op, Principal, Room, RoomError, Store, StoreError};
use meldframe_http::{Failure, Guard, Submitter};
use meldframe_http_live::{LiveSource, Opened, Out};
use meldframe_http_snapshot::{FileSnapshot, Snapshots};
use meldframe_module::Policy;
use meldframe_protocol::ServerMsg;
use meldframe_rooms::RegistryError;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Mutex, MutexGuard};

/// A [`Server`] shared by the transport's blocking calls. One writer at a time, as the room actor assumes.
#[derive(Debug)]
pub struct SharedServer(Mutex<Server>);

impl SharedServer {
    pub fn new(server: Server) -> Self {
        SharedServer(Mutex::new(server))
    }

    fn lock(&self) -> MutexGuard<'_, Server> {
        // A panic inside one call must not take every later call down with it. Each accepted op is already a
        // durable write, but the rooms in memory are as the panic left them: close them all, so each is
        // restored from the store on its next use, and clear the poison so this happens once.
        self.0.lock().unwrap_or_else(|p| {
            let mut server = p.into_inner();
            server.close_all();
            self.0.clear_poison();
            server
        })
    }

    /// Send an ephemeral frame (a capture request, a touch hint) to one stream of `file` in the principal's
    /// workspace, or to all of them; returns how many streams queued it. Open sockets pick it up on their
    /// next look at the stream.
    pub fn publish_ephemeral(
        &self,
        who: &Principal,
        file: &str,
        to: Option<StreamId>,
        msg: ServerMsg,
    ) -> usize {
        self.lock().publish_ephemeral(who, file, to, msg)
    }

    /// Read access for the embedder and tests; there is no write path here.
    pub fn with_server<R>(&self, f: impl FnOnce(&Server) -> R) -> R {
        f(&self.lock())
    }
}

impl Submitter for SharedServer {
    fn submit(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure> {
        self.lock()
            .submit_tool(tool, file, who, ops)
            .map_err(|e| failure_of(&e))
    }

    /// All or nothing: the batch is checked against the room (`meldframe_batch::check`) under the same lock
    /// that then submits it, so no other writer can slip in between. Then it goes through the gate as usual.
    fn submit_atomic(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
        guards: Vec<Guard>,
    ) -> Result<Vec<Accepted>, Failure> {
        let mut s = self.lock();
        let (ws, was_open) = (
            who.workspace.as_str(),
            s.with_room(&who.workspace, file, |_| ()),
        );
        let was_open = was_open.is_some();
        s.open(ws, file)
            .map_err(|e| failure_of(&SubmitError::Rooms(e)))?;
        let checked = s
            .with_room(ws, file, |room| {
                meldframe_batch::check(room, &who.actor, &ops, &guards)
            })
            .expect("the room was just opened");
        let done = match checked {
            Err(Stop::Refused { index, refusal }) => Err(Failure::Batch { index, refusal }),
            Err(Stop::Store(e)) => Err(Failure::Unavailable(e.to_string())),
            Ok(()) => s
                .submit_tool(tool, file, who, ops)
                .map_err(|e| failure_of(&e)),
        };
        if done.is_err() && !was_open {
            s.close(ws, file);
        }
        done
    }
}

impl SharedServer {
    /// Run `f` on the room of `file` in the principal's workspace, opening it for the call when it is closed.
    /// A room opened for the call is closed again even when `f` panics.
    fn with_open_room<R>(
        &self,
        who: &Principal,
        file: &str,
        f: impl FnOnce(&Room<AnyStore>) -> R,
    ) -> Result<R, Failure> {
        let mut s = self.lock();
        let was_open = s.with_room(&who.workspace, file, |_| ()).is_some();
        if !was_open {
            s.open(&who.workspace, file)
                .map_err(|e| failure_of(&SubmitError::Rooms(e)))?;
        }
        let out = catch_unwind(AssertUnwindSafe(|| {
            s.with_room(&who.workspace, file, f)
                .expect("the room is open")
        }));
        if !was_open {
            s.close(&who.workspace, file);
        }
        match out {
            Ok(v) => Ok(v),
            Err(panic) => resume_unwind(panic),
        }
    }

    /// Like `with_open_room`, but only for a read tool the registry has with an `Auto` policy; without the
    /// MCP module nothing reads (the same rule for the MCP reads and the snapshot route).
    fn read<R>(
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
}

/// The snapshot route reads the file of the principal's workspace (the token decides the workspace) under the
/// policy of the `get_tree` read tool, like the MCP reads: a registry that does not allow that read allows
/// no snapshot either. Every call of a closed file opens and closes its room; there is no cache or per-workspace
/// cap on that yet, only the per-principal budget and the in-flight slots of the adapter.
impl Snapshots for SharedServer {
    fn snapshot(&self, who: &Principal, file: &str) -> Result<FileSnapshot, Failure> {
        self.read("get_tree", who, file, |r| FileSnapshot {
            seq: r.seq(),
            nodes: r.nodes().to_vec(),
        })
    }
}

#[cfg(feature = "mcp")]
mod read {
    use super::*;
    use meldframe_host::StoredOp;
    use meldframe_http::mcp::{Conflict, FileView, Reader};
    fn unavailable(e: StoreError) -> Failure {
        Failure::Unavailable(e.to_string())
    }

    impl Reader for SharedServer {
        fn view(&self, who: &Principal, file: &str) -> Result<FileView, Failure> {
            self.read("get_tree", who, file, |r| FileView {
                seq: r.seq(),
                nodes: r.nodes().to_vec(),
            })
        }

        fn tree(
            &self,
            who: &Principal,
            file: &str,
            window: usize,
        ) -> Result<(FileView, Vec<StoredOp>), Failure> {
            self.read("get_tree", who, file, |r| {
                let from = r.seq().saturating_sub(window as u64);
                let recent = r.store().ops_after(r.id(), from, window)?;
                Ok((
                    FileView {
                        seq: r.seq(),
                        nodes: r.nodes().to_vec(),
                    },
                    recent,
                ))
            })?
            .map_err(unavailable)
        }

        fn conflict(
            &self,
            who: &Principal,
            file: &str,
            since: u64,
            node: &str,
            limit: usize,
        ) -> Result<Conflict, Failure> {
            self.read("get_tree", who, file, |r| {
                let changes = meldframe_batch::changes_to(r, since, node, limit)?;
                Ok(Conflict {
                    seq: r.seq(),
                    node: r.nodes().iter().find(|n| n.id == node).cloned(),
                    changes,
                })
            })?
            .map_err(unavailable)
        }

        fn applied(
            &self,
            who: &Principal,
            file: &str,
            keys: &[String],
        ) -> Result<Vec<Option<u64>>, Failure> {
            self.read("batch_status", who, file, |r| {
                keys.iter()
                    .map(|k| r.store().find(r.id(), &who.actor, k))
                    .collect::<Result<Vec<_>, _>>()
            })?
            .map_err(unavailable)
        }
    }
}

/// What the transport is told about a failed submit: a refusal, a store error that can never succeed (the
/// text starts with [`PERMANENT_STORE_ERROR`], which only `PgStore` writes), or anything else as unavailable.
pub fn failure_of(e: &SubmitError) -> Failure {
    if let Some(r) = e.refusal() {
        return Failure::Refused(r.clone());
    }
    match e {
        SubmitError::Rooms(RegistryError::Room(RoomError::Store(StoreError::Unavailable(m))))
            if m.starts_with(PERMANENT_STORE_ERROR) =>
        {
            Failure::Permanent(e.to_string())
        }
        _ => Failure::Unavailable(e.to_string()),
    }
}

/// The live streams of the server, for the WebSocket route of the adapter.
impl LiveSource for SharedServer {
    fn subscribe(
        &self,
        who: &Principal,
        file: &str,
        from_seq: u64,
        now: u64,
    ) -> Result<Opened, Failure> {
        let sub = self.lock().subscribe(who, file, from_seq, now);
        opened(sub)
    }

    fn resubscribe(
        &self,
        who: &Principal,
        file: &str,
        id: StreamId,
        from_seq: u64,
        now: u64,
    ) -> Result<Opened, Failure> {
        let sub = self.lock().resubscribe(who, file, id, from_seq, now);
        opened(sub)
    }

    fn poll(&self, who: &Principal, file: &str, id: StreamId, now: u64, max: usize) -> Vec<Out> {
        let mut server = self.lock();
        std::iter::from_fn(|| server.poll(who, file, id, now))
            .take(max)
            .collect()
    }

    fn seen(&self, who: &Principal, file: &str, id: StreamId, now: u64) {
        self.lock().seen(who, file, id, now);
    }

    fn unsubscribe(&self, who: &Principal, file: &str, id: StreamId) {
        self.lock().unsubscribe(who, file, id);
    }

    fn heartbeat_ms(&self) -> Option<u64> {
        Some(self.lock().live_heartbeat_ms())
    }

    fn publish_ephemeral(&self, who: &Principal, file: &str, to: StreamId, msg: ServerMsg) -> bool {
        self.lock().publish_ephemeral(who, file, Some(to), msg) > 0
    }

    fn tick(&self, now: u64) -> Vec<StreamId> {
        // The file keys name every workspace's files: only the ids leave this function.
        let dropped = self.lock().tick(now);
        dropped.into_iter().map(|(_, id)| id).collect()
    }
}

/// A failed subscribe is not a refusal of the client's call: the stream cap, the registry and the store are
/// the server's to answer for, so the transport says `unavailable` (retryable).
fn opened(sub: Result<Subscription, SubscribeError>) -> Result<Opened, Failure> {
    sub.map(|s| Opened {
        id: s.id,
        replay: s.replay,
    })
    .map_err(|e| Failure::Unavailable(format!("{e:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_opened_for_a_read_is_closed_even_when_the_read_panics() {
        let server = SharedServer::new(Server::assemble());
        let who = Principal {
            workspace: "ws".into(),
            actor: "usr".into(),
        };
        let panicked = catch_unwind(AssertUnwindSafe(|| {
            let _ = server.with_open_room(&who, "f", |_| panic!("boom"));
        }));
        assert!(panicked.is_err());
        assert!(
            server
                .with_server(|s| s.with_room("ws", "f", |_| ()))
                .is_none()
        );
    }
}
