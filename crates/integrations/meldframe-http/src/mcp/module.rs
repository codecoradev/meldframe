//! The registration of the MCP tools in the registry, with their tiers.

use super::TOOLS;
use meldframe_module::{Module, Registrar};

/// Registers the MCP tools in the registry with their tiers (`apply_commands` is a Change, the rest Reads).
/// Without it `apply_commands` is refused as `forbidden`.
#[derive(Debug, Clone, Copy)]
pub struct McpModule;

impl Module for McpModule {
    fn name(&self) -> &'static str {
        "mcp"
    }

    fn register(&self, r: &mut Registrar) {
        for t in &TOOLS {
            r.agent_tool(t.name, t.description, t.tier);
        }
    }
}
