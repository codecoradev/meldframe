//! The two calls of a person, `checkpoint_create` and `checkpoint_state`, and the module that registers them.

use meldframe_module::{Module, Registrar, RiskTier};

/// The call that makes a checkpoint, a person's: a Change, not offered over MCP.
pub const CREATE_TOOL: &str = "checkpoint_create";
pub const CREATE_DESCRIPTION: &str = "Make a checkpoint of a file: a labelled point in its history that a person can restore. A person's decision, made through the route POST /v1/files/{file}/checkpoints; not offered over MCP.";
/// The call that reads a file as it was at a checkpoint, a person's: a Read, not offered over MCP.
pub const STATE_TOOL: &str = "checkpoint_state";
pub const STATE_DESCRIPTION: &str = "Read a file as it was at a checkpoint. A person's read, made through the route GET /v1/files/{file}/checkpoints/{n}/state; not offered over MCP.";

/// Registers the two calls in the registry. Without it the registry has neither.
#[derive(Debug, Clone, Copy)]
pub struct CheckpointsModule;

impl Module for CheckpointsModule {
    fn name(&self) -> &'static str {
        "checkpoints"
    }

    fn register(&self, r: &mut Registrar) {
        r.agent_tool(CREATE_TOOL, CREATE_DESCRIPTION, RiskTier::Change);
        r.agent_tool(STATE_TOOL, STATE_DESCRIPTION, RiskTier::Read);
    }
}
