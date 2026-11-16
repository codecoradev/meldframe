//! Example feature module that shows the feature layout.

use meldframe_module::{Module, Registrar, RiskTier};

/// The placeholder module. It registers a Read-tier and a Change-tier agent tool.
#[derive(Debug, Clone, Copy)]
pub struct ExampleModule;

impl Module for ExampleModule {
    fn name(&self) -> &'static str {
        "example"
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.agent_tool("example_ping", "Answer with a fixed reply.", RiskTier::Read);
        // Lets a developer write through the gate (REST, WebSocket) before a real module registers one.
        registrar.agent_tool(
            "example_edit",
            "Submit ops that change the file, as a placeholder for a real editing tool.",
            RiskTier::Change,
        );
    }
}
