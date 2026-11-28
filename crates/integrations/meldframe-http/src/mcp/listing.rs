//! How a tool is listed to a client: its name, description, schema and annotations.

use meldframe_mcp_defs::ToolDef;
use rmcp::model::{Tool, ToolAnnotations};
use serde_json::Value;

/// The listing of a tool that another crate contributes and that writes to a file: not read-only, not destructive,
/// idempotent (every call carries an idempotency key). `schema` is the JSON Schema of its arguments.
pub fn change_tool_listing(name: &'static str, description: &'static str, schema: Value) -> Tool {
    let schema = match schema {
        Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    Tool::new(name, description, schema).annotate(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(true),
    )
}

pub(super) fn tool_of(d: &ToolDef) -> Tool {
    let schema = match d.input_schema() {
        Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    let read_only = d.tier == meldframe_module::RiskTier::Read;
    Tool::new(d.name, d.description, schema).annotate(
        ToolAnnotations::new()
            .read_only(read_only)
            .destructive(false)
            .idempotent(true),
    )
}
