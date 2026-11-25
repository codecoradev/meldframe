//! The MCP tool `import_html` (E4.4, `docs/import-html.md` section 4, ADR-0024): HTML and CSS text to the `addNode`
//! Commands of one atomic batch.
//!
//! The text is read with the `html-import` feature, a function of the text that opens nothing and runs nothing. The
//! arguments are checked whole first ([`args`]), the plan is made on the blocking pool under the call's slot ([`plan`])
//! and the Commands are written with `Shared::submit_ops`, the handler every transport ends in: the same gate, quota,
//! limits and atomic batch as `apply_commands`.

mod answer;
mod args;
mod file;
mod plan;
mod schema;
mod tool;

pub use answer::describe;
pub use args::Args;
pub use plan::{Planned, make};
pub use schema::{DESCRIPTION, MAX_IMPORT_KEY, TOOL, input_schema};
pub use tool::tool;

use meldframe_module::{Module, Registrar, RiskTier};
use serde_json::{Value, json};

/// The tool registers as a Change: it adds nodes to the file. Without the module the gate refuses the write as
/// `forbidden`, and a build without the `html-import` feature of the server has no such tool.
#[derive(Debug, Clone, Copy)]
pub struct HtmlImportModule;

impl Module for HtmlImportModule {
    fn name(&self) -> &'static str {
        "html-import"
    }

    fn register(&self, r: &mut Registrar) {
        r.agent_tool(TOOL, DESCRIPTION, RiskTier::Change);
    }
}

/// The body of a tool error.
pub(crate) fn refuse(code: &str, message: impl Into<String>) -> Value {
    json!({"ok": false, "error": {"code": code, "message": message.into(), "retryable": false}})
}

pub(crate) fn invalid(message: impl Into<String>) -> Value {
    refuse("invalid", message)
}

/// A refused call, as the one handler words it: the same code, message and extras as REST.
pub(crate) fn failed(reply: &meldframe_http::Reply) -> Value {
    json!({"ok": false, "error": reply.body["error"]})
}
