//! The relay of one server: [`Capture`] and the [`Options`] the server decides around it.

use crate::hook;
use crate::hub::Hub;
use crate::tool;
use crate::upload;
use axum::Router;
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
