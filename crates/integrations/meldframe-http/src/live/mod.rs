//! The live WebSocket of the HTTP adapter (ADR-0009, E1.4): `GET /v1/live/{file}?after={seq}`.
//!
//! It is its own socket, not a message on `/v1/ws`: the handshake can answer 401, 429 or 503 as a status, one
//! connection is one stream with one lifetime (so its heartbeat, reaping and ownership need no routing table
//! of requests), and the submit socket stays plain request and reply. The protocol is in `ws.rs`.
//!
//! It builds on `meldframe-http`: the same authenticator, per-address and per-principal budgets and size
//! limit through [`Shared`], and a hook on [`Shared::on_write`] that wakes the sockets of a file when an op of
//! it was accepted over REST, `/v1/ws` or MCP. [`routes`] adds the route to any router built over the same
//! `Shared`, and returns the [`Timer`] to spawn; [`live_router`] does both steps for the REST and `/v1/ws`
//! routes. Without the timer no heartbeat is sent and a vanished client keeps its stream.

mod config;
mod hub;
mod routes;
mod ws;

pub use config::LiveConfig;
pub use hub::{ClientHook, LiveSource, Opened, Out, Timer};
pub(crate) use routes::LiveState;
pub use routes::{live_router, routes};
