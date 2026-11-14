//! Module trait, registry and risk tiers.
//!
//! This crate is portable: no IO, no async runtime, and no import of any feature crate.

mod tier;

pub use tier::{Policy, RiskTier, default_policy, fallback_policy};
