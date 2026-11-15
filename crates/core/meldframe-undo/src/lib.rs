//! The bookkeeping behind undo and redo, with no tree in it: the steps of a history (`Rec`, `Entry`),
//! the stacks and runs that hold them (`History`), the rules that rewrite them against a remote
//! command (`History::rewrite`) and helpers over `NodePatch`. `meldframe-history` keeps the tree (`FileState`),
//! works out the commands and drives this crate.

mod history;
mod patch_fields;
mod patch_ops;
mod remote;
mod runs;

pub use history::{Entry, History, Rec};
pub use patch_ops::{compact, drop_fields, inverse_patch, patch_is_empty};
pub use remote::RemoteCtx;
