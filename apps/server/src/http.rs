//! The server behind the HTTP adapter: [`SharedServer`] is the one [`Submitter`], so REST and WebSocket reach
//! a room only through `Server::submit_tool` (the gate).

use crate::{PERMANENT_STORE_ERROR, Server, SubmitError};
use meldframe_host::{Accepted, Op, Principal, RoomError, StoreError};
use meldframe_http::{Failure, Submitter};
use meldframe_rooms::RegistryError;
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
