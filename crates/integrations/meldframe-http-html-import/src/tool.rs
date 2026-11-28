//! The tool object the server mounts: its listing, its call, and the state every call reads.

use super::schema::input_schema;
use html_import::{DESCRIPTION, TOOL};
use meldframe_host::Principal;
use meldframe_http::Shared;
use meldframe_http::mcp::{Extra, ExtraCall, Reader, change_tool_listing};
use meldframe_module::Policy;
use rmcp::model::{CallToolResult, Tool};
use serde_json::{Map, Value};
use std::sync::Arc;

/// A tool's answer: the body of a success, or the body of a tool error.
pub(super) type Body = Result<Value, Value>;

pub(super) struct HtmlImport {
    pub(super) shared: Arc<Shared>,
    pub(super) reader: Arc<dyn Reader>,
    pub(super) policy: Option<Policy>,
}

impl std::fmt::Debug for HtmlImport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HtmlImport")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

/// The tool for `McpConfig::extra`. `policy` is what the registry says for [`TOOL`]; without the module (`None`) the
/// tool serves nothing. `reader` places the root beside the other frames and finds what a retry wrote; `shared` runs
/// the work under the slots and the time limit of every other call and ends in the one handler of writes.
pub fn tool(
    shared: Arc<Shared>,
    reader: Arc<dyn Reader>,
    policy: Option<Policy>,
) -> Arc<dyn Extra> {
    Arc::new(HtmlImport {
        shared,
        reader,
        policy,
    })
}

impl Extra for HtmlImport {
    fn tool(&self) -> Tool {
        change_tool_listing(TOOL, DESCRIPTION, input_schema())
    }

    fn call(&self, who: Principal, args: Map<String, Value>) -> ExtraCall<'_> {
        Box::pin(async move {
            match self.run(&who, args).await {
                Ok(v) => CallToolResult::structured(v),
                Err(v) => CallToolResult::structured_error(v),
            }
        })
    }
}
