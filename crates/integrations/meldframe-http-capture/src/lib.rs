//! The capture relay over HTTP (E1.7). One [`Capture`] holds the relay state and gives three parts that
//! share it: the MCP tool ([`Capture::tool`], for `McpConfig::extra`), the hook on the live sockets
//! ([`Capture::hook`], for `LiveConfig::hook`) and the upload route ([`Capture::routes`]).
//!
//! The flow of `capture {file?, nodeId?, scale?}`:
//! 1. The file is the one bound to the caller (workspace and actor, so one agent token is one session). A call
//!    that names a file and finds an editor for it binds it (a call that fails before an editor was found, a
//!    typo for one, binds nothing); a later call that names another file is `room_mismatch`; a call that names
//!    none on an unbound caller is `no_file_selected`. A binding lasts until the server restarts.
//! 2. An editor is capable when its live socket sent `{"type":"capabilities","room":<file>,"capabilities":
//!    ["capture"]}` after the handshake. With none open on that file of the caller's workspace the call fails
//!    at once with `no_capable_client`.
//! 3. The relay sends that socket the ephemeral `captureRequest` (it carries a random upload token and the
//!    file's sequence number as `minSeq`); the call waits.
//! 4. The editor renders offscreen and sends `POST /captures/{requestId}` with `Authorization: Bearer
//!    <its token>`, `X-Capture-Token: <uploadToken>`, `X-Capture-Seq: <sequence number it rendered>` and the
//!    PNG as the body, or answers on the socket with `captureFailed`. The route first checks that the request
//!    id and token match a pending request (else 404, the body is never read), takes an upload place, then reads
//!    the body as a stream, never beyond the size limit (`Relay::max_bytes`); the `Content-Length` is checked first.
//! 5. The agent gets the PNG as MCP image content, or a tool error whose code is one of `capture::CODES`.
//!
//! An answer without a matching request (unknown id, wrong token, another workspace, another client) is
//! ignored: the upload answers 404 and the socket message is dropped. A capture that gets no answer fails
//! with `timeout` at its deadline, which the live timer checks.
//!
//! Limits: a full relay or a client at its limit is `busy`; a capable client past the cap is not registered.

mod hook;
mod hub;
mod tool;
mod upload;

pub use tool::{NAME, input_schema};

use axum::Router;
use hub::Hub;
use meldframe_http::live::{ClientHook, LiveSource};
use meldframe_http::mcp::{Extra, Reader};
use meldframe_http::{Clock, Shared};
use std::sync::Arc;

/// What the server decides around the relay.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Uploads being read at once, in all.
    pub uploads: usize,
    /// The same, per principal.
    pub uploads_per_principal: usize,
    /// The registry's policy for the `capture` tool: only `Some(Policy::Auto)` lets a call run (a Read is
    /// Auto by default; no readable policy, Ask and Deny refuse it with `forbidden`).
    pub policy: Option<meldframe_module::Policy>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            uploads: 8,
            uploads_per_principal: 2,
            policy: Some(meldframe_module::Policy::Auto),
        }
    }
}

/// The capture relay of one server.
#[derive(Debug, Clone)]
pub struct Capture {
    hub: Arc<Hub>,
}

impl Capture {
    /// `source` publishes the request to an editor's stream and `reader` tells the file's sequence number.
    /// `clock` is the adapter's clock, the one the live timer uses.
    pub fn new(
        shared: Arc<Shared>,
        source: Arc<dyn LiveSource>,
        reader: Arc<dyn Reader>,
        clock: Clock,
        config: capture::Config,
        options: Options,
    ) -> Capture {
        Capture {
            hub: Arc::new(Hub::new(shared, source, reader, clock, config, options)),
        }
    }

    /// The hook for `LiveConfig::hook`.
    pub fn hook(&self) -> Arc<dyn ClientHook> {
        Arc::new(hook::Hook(self.hub.clone()))
    }

    /// The tool for `McpConfig::extra`.
    pub fn tool(&self) -> Arc<dyn Extra> {
        Arc::new(tool::CaptureTool(self.hub.clone()))
    }

    /// `POST /captures/{requestId}`, to merge into the router.
    pub fn routes(&self) -> Router {
        upload::routes(self.hub.clone())
    }
}
