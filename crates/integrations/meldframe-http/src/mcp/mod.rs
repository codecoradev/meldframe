//! MCP over streamable HTTP (`/mcp`), the third transport of the one gate.
//!
//! It is stateless: every request carries its own Bearer token, and the principal of a tool call is the one
//! that request authenticated as.
//!
//! Writes end in `Shared::submit_ops`, the handler REST and WebSocket end in (same authenticator, limits, slots
//! and submitter); reads go through the [`Reader`] seam on the same blocking pool. The tool text and schemas
//! are in [`TOOLS`]; `docs/mcp-tools.json` lists them and a test compares.

mod extra;
mod handler;
mod hash;
mod listing;
mod module;
mod reader;
mod route;
mod tools;
mod variables;

pub use extra::{Extra, ExtraCall};
pub use hash::fnv1a;
pub use listing::change_tool_listing;
pub use meldframe_mcp_defs::{MAX_BATCH, MAX_KEY, TOOLS, ToolDef, check_key, key_schema};
pub use module::McpModule;
pub use reader::{Conflict, FileView, Reader};
pub use route::{McpConfig, routes};
/// The body of a tool error and of a refused call, as the tools of this crate word them, for the tools of other crates
/// (`make_variant`, `import_html`): one wording, one shape.
pub use tools::{error as tool_error, failed as tool_failed};

pub(crate) use handler::Handler;
