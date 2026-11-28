//! The capture relay over HTTP (E1.7): a tool, a hook on the live sockets and an upload route over one [`Capture`].
//!
//! One [`Capture`] holds the relay state and gives three parts that share it: the MCP tool ([`Capture::tool`], for
//! `McpConfig::extra`), the hook on the live sockets ([`Capture::hook`], for `LiveConfig::hook`) and the upload route
//! ([`Capture::routes`]).
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
mod relay;
mod tool;
mod upload;

pub use relay::{Capture, Options};
pub use tool::{NAME, input_schema};
