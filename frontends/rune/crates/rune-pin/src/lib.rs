//! The Rune revision this workspace is built against, written once in code.
//!
//! Rune is used by git revision only (ADR-0027). The manifest, the lock and `deny.toml` name the same
//! revision, and the tests of this crate fail when one of them does not. The crate links `rune` so that
//! `cargo check` compiles the pinned revision with its eight patches.

/// The git URL of Rune.
pub const RUNE_URL: &str = "https://github.com/anomalyco/rune";

/// The full revision of Rune (`048528dc35f0` is its first 12 characters).
pub const RUNE_REVISION: &str = "048528dc35f0a7499887f334330fd9704cdd8e51";

/// The name of a type of the pinned Rune, so that this crate does use it.
pub fn linked_app_spec() -> &'static str {
    std::any::type_name::<rune::AppSpec<()>>()
}
