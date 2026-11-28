//! The shared server: one [`Server`] behind a lock, for the blocking calls of the transport.

use super::failure_of;
use crate::{AnyStore, Server, StreamId, SubmitError};
use meldframe_host::{Principal, Room};
use meldframe_http::Failure;
use meldframe_protocol::ServerMsg;
use std::sync::{Mutex, MutexGuard};

/// A [`Server`] shared by the transport's blocking calls. One writer at a time, as the room actor assumes.
#[derive(Debug)]
pub struct SharedServer(Mutex<Server>);

impl SharedServer {
    pub fn new(server: Server) -> Self {
        SharedServer(Mutex::new(server))
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, Server> {
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

    /// The newest `limit` entries of the call log, oldest first.
    pub fn calls(&self, limit: usize) -> Vec<meldframe_calls::CallEntry> {
        self.lock().calls(limit)
    }

    pub fn pending_approvals(&self, workspace: &str) -> Vec<meldframe_approvals::Approval> {
        self.lock().pending_approvals(workspace)
    }

    /// The owner's answer to an approval (see `Server::decide_approval`).
    pub fn decide_approval(
        &self,
        who: &Principal,
        id: &str,
        approved: bool,
    ) -> Result<meldframe_approvals::ApprovalState, meldframe_approvals::AnswerError> {
        self.lock().decide_approval(who, id, approved)
    }

    /// The bytes of ops a workspace has written and its storage limit (for the asset quota).
    pub fn ops_storage(&self, workspace: &str) -> (u64, Option<u64>) {
        self.lock().ops_storage(workspace)
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

    /// [`Server::publish_with_headroom`]: an ephemeral frame to the streams of `file` that keep headroom.
    pub fn publish_with_headroom(&self, who: &Principal, file: &str, msg: ServerMsg) -> usize {
        self.lock().publish_with_headroom(who, file, msg)
    }

    /// Read access for the embedder and tests; there is no write path here.
    pub fn with_server<R>(&self, f: impl FnOnce(&Server) -> R) -> R {
        f(&self.lock())
    }
}

impl SharedServer {
    /// Run `f` on the room of `file`, opening it for the call when it is closed (and closing it after).
    pub(super) fn with_open_room<R>(
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
        let out = s
            .with_room(&who.workspace, file, f)
            .expect("the room is open");
        if !was_open {
            s.close(&who.workspace, file);
        }
        Ok(out)
    }
}
