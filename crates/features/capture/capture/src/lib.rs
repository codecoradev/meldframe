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

/// The tool text an agent reads (English, part of the contract; `docs/mcp-tools.json` lists it).
pub const DESCRIPTION: &str = "Capture a node, or the whole page, of the file bound to your session as a PNG image, rendered by an editor that has the file open. Name the file on your first call: the session stays bound to it, and naming another file later is refused with room_mismatch (a call that names no file on a session with none bound is refused with no_file_selected). Pass nodeId to capture one node (leave it out for the whole page) and scale, above 0 and at most 4 (default 1); the image is at most 4 MiB. With no editor open on the file that can capture, the call fails at once with no_capable_client; an editor that does not answer in time fails it with timeout, an image over the size limit with too_large, and an editor that has not caught up to the file with stale_client. The image comes back as image content. The image shows the border and the shadows of the nodes, and a capture of a node is larger than the node by the band of its outside border and the reach of its shadows (offset, spread and blur). A node with no fill, a translucent fill, or an opacity below 1 shows its own shadow through itself in the image, which the exported page does not. A whole-page capture grows to include shadows and children that lie outside the page and can then be refused as too_large; capture the node by nodeId instead. Changes nothing.";

/// The capture module. It registers one Read-tier agent tool: a capture changes nothing.
#[derive(Debug, Clone, Copy)]
pub struct CaptureModule;

impl Module for CaptureModule {
    fn name(&self) -> &'static str {
        "capture"
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.agent_tool("capture", DESCRIPTION, RiskTier::Read);
    }
}
