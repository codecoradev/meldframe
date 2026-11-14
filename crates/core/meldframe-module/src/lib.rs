//! Module trait, registry and risk tiers.
//!
//! This crate is portable: no IO, no async runtime, and no import of any feature crate. A feature
//! module implements [`Module`] in its own crate; the composition root registers it in a
//! [`Registry`] that the editor, MCP and REST read (ADR-0007).

mod registry;
mod tier;

pub use registry::{AgentTool, Entry, Module, Registrar, Registry, RegistryError, Surface};
pub use tier::{Policy, RiskTier, default_policy, fallback_policy};
