//! The server side of one MCP request: the handler that lists the tools and routes a call to its tool.

use super::listing::tool_of;
use super::{Extra, Reader, TOOLS};
use crate::shared::Shared;
use meldframe_host::Principal;
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::Value;
use std::sync::Arc;

/// The per-request state of a tool call: the shared handler state and the reader.
#[derive(Clone)]
pub(crate) struct Handler {
    pub(crate) shared: Arc<Shared>,
    pub(crate) reader: Arc<dyn Reader>,
    pub(crate) extra: Vec<Arc<dyn Extra>>,
}

impl Handler {
    /// One entry in the call log through the reader, in a slot of the principal with the call time limit, like the
    /// other calls of the server (a busy server loses the entry rather than holding the reply).
    async fn record(&self, who: Principal, tool: String, file: String, code: Option<String>) {
        let (reader, w) = (self.reader.clone(), who.clone());
        let job = move || reader.record(&w, &tool, &file, code.as_deref());
        let _ = self.shared.blocking(&who, job).await;
    }
}

impl ServerHandler for Handler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Meldframe design files. Call whoami first, read with get_tree, get_nodes and get_variables, change with apply_commands (atomic, idempotent, ifSeq-guarded) and check a call with batch_status. Other tools the server lists are described there.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(
            (TOOLS.iter().map(tool_of))
                .chain(self.extra.iter().map(|x| x.tool()))
                .collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let who = context
            .extensions
            .get::<axum::http::request::Parts>()
            .and_then(|p| p.extensions.get::<Principal>())
            .cloned()
            .ok_or_else(|| ErrorData::internal_error("the call carries no principal", None))?;
        let a = request.arguments.unwrap_or_default();
        if let Some(x) = self.extra.iter().find(|x| x.tool().name == request.name) {
            // The log holds a file name only when it is one: what a caller sent that is not is logged as no file.
            let file = a
                .get("file")
                .and_then(Value::as_str)
                .filter(|f| crate::valid_file(f))
                .unwrap_or("")
                .to_owned();
            let out = x.call(who.clone(), a).await;
            let code = (out.is_error == Some(true)).then(|| {
                out.structured_content
                    .as_ref()
                    .and_then(|v| v["error"]["code"].as_str())
                    .unwrap_or("internal")
                    .to_owned()
            });
            self.record(who, request.name.to_string(), file, code).await;
            return Ok(out.into());
        }
        let body = match request.name.as_ref() {
            "whoami" => {
                let body = self.whoami(&who);
                self.record(who.clone(), "whoami".into(), String::new(), None)
                    .await;
                body
            }
            "get_tree" => self.get_tree(&who, a).await,
            "get_nodes" => self.get_nodes(&who, a).await,
            "get_variables" => self.get_variables(&who, a).await,
            "apply_commands" => self.apply_commands(&who, a).await,
            "batch_status" => self.batch_status(&who, a).await,
            other => {
                return Err(ErrorData::invalid_params(
                    format!("unknown tool {other}; list the tools with tools/list"),
                    None,
                ));
            }
        };
        Ok(match body {
            Ok(v) => CallToolResult::structured(v),
            Err(v) => CallToolResult::structured_error(v),
        }
        .into())
    }
}
