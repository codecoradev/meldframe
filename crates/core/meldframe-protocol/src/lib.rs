//! Wire and model types shared by server, client and agents.
//!
//! This crate is the source of truth: the TypeScript types in `packages/protocol/src/generated` are produced from it
//! by ts-rs (`cargo test -p meldframe-protocol` writes them; `tools/check-protocol.sh` fails on drift).
//!
//! One enum, [`Command`], is the UI command, the undo record, the WebSocket op and the agent tool argument.
//! Deserialization is deliberately strict (`deny_unknown_fields`): an unknown field is refused, never ignored.
//! What an older client may omit is optional with a default, so it stays valid.

mod command;
mod decoration;
mod enums;
mod node;
mod patch;
mod snapshot;
mod variables;
mod version;
mod wire;

pub use command::Command;
pub use decoration::{Border, Gradient, GradientStop, ImageFill, Shadow};
pub use enums::{
    Align, BorderPosition, GradientKind, ImageFit, Justify, LayoutMode, NodeKind, ShaderPreset,
    SizeMode, TextAlign,
};
pub use node::{Node, Pad};
pub use patch::NodePatch;
pub use snapshot::{FileSnapshot, SnapshotNode};
pub use variables::{Bind, Collection, Doc, Mode, Value, VarRef, VarType, Variable, Variables};
pub use version::SCHEMA_VERSION;
pub use wire::{ClientMsg, MAX_OP_DEPS, ServerMsg};
