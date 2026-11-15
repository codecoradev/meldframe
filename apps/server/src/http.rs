//! The server behind the HTTP adapter: [`SharedServer`] is the one [`Submitter`], so REST and WebSocket reach
//! a room only through `Server::submit_tool` (the gate).

use crate::{Server, SubmitError};
use meldframe_host::{Accepted, Op, Principal};
use meldframe_http::{Failure, Submitter};
use std::sync::{Mutex, MutexGuard};

/// A [`Server`] shared by the transport's blocking calls. One writer at a time, as the room actor assumes.
#[derive(Debug)]
pub struct SharedServer(Mutex<Server>);

impl SharedServer {
    pub fn new(server: Server) -> Self {
        SharedServer(Mutex::new(server))
    }

    fn lock(&self) -> MutexGuard<'_, Server> {
        // A panic inside one call must not take every later call down with it: each accepted op is already
        // a durable write, so the server's state is still the log.
        self.0.lock().unwrap_or_else(|p| p.into_inner())
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
            .map_err(|e| match e.refusal() {
                Some(r) => Failure::Refused(r.clone()),
                None => Failure::Unavailable(describe(&e)),
            })
    }
}

fn describe(e: &SubmitError) -> String {
    e.to_string()
}
