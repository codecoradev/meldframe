//! Example feature module that shows the feature layout.

use meldframe_module::{Module, Registrar, RiskTier};

/// The placeholder module. It registers one Read-tier agent tool.
#[derive(Debug, Clone, Copy)]
pub struct ExampleModule;

impl Module for ExampleModule {
    fn name(&self) -> &'static str {
        "example"
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.agent_tool("example_ping", "Answer with a fixed reply.", RiskTier::Read);
    }
}
