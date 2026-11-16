//! Capture feature module: the `capture` agent tool and the relay that hands a request for an image to
//! an open editor and takes its answer back (E1.7).
//!
//! The relay is pure and synchronous (no transport, no async runtime, no clock: every call that needs time
//! takes `now` in milliseconds), so it is portable and tested without a socket. The transport (the live
//! WebSocket and `POST /captures/{requestId}`) feeds it and carries its frames.
//!
//! Protocol (the wire types live in `meldframe-protocol`):
//! 1. A capture-capable client advertises it with `ClientMsg::Capabilities { room, capabilities: ["capture"] }`
//!    on a stream it subscribed. A client that advertises it must answer every request.
//! 2. The agent's call becomes [`Relay::begin`]: the relay picks one capable client of the same workspace and
//!    file and returns the ephemeral `ServerMsg::CaptureRequest` to publish to that stream only.
//! 3. The client renders offscreen and uploads the PNG ([`Relay::upload`]) or refuses with
//!    `ClientMsg::CaptureFailed` ([`Relay::failed`]).
//! 4. [`Relay::expire`] fails requests past their deadline with `timeout`.
//!
//! `Begin.file` and the `room` of the request are the namespaced room string the client subscribed with.
//! The HTTP layer must enforce the body limit (`Relay::max_bytes`) while it streams the upload and must not
//! buffer more; `Relay::upload` checks the length before it copies anything.
//!
//! The codes are listed in `docs/error-codes.md` (section "Capture codes").

mod binding;
mod error;
mod relay;
mod token;

pub use binding::Binding;
pub use error::{CODES, CaptureError};
pub use relay::{Begin, Config, Done, Relay};
pub use token::{Token, ct_eq};

use meldframe_module::{Module, Registrar, RiskTier};

/// The capture module. It registers one Read-tier agent tool: a capture changes nothing.
#[derive(Debug, Clone, Copy)]
pub struct CaptureModule;

impl Module for CaptureModule {
    fn name(&self) -> &'static str {
        "capture"
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.agent_tool(
            "capture",
            "Capture a node, or the whole page, of the bound file as a PNG, through an open editor.",
            RiskTier::Read,
        );
    }
}
