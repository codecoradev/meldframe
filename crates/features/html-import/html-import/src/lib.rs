//! HTML import feature module (E4.4, ADR-0024, `docs/import-html.md`): it reads HTML text that an agent sends and
//! turns it into Nodes. This crate is the facade: it names the stages under the paths the callers use and holds the
//! two that make nodes. The stages before them are crates of their own so that none passes the size ceiling:
//!
//! - [`html`] is `html-import-tree`, the tolerant, bounded tree builder;
//! - [`css`] is `html-import-css`, the reader and the cascade, with `html-import-style`, what is done with the
//!   declarations;
//! - [`tw`] is the Tailwind decoder of `html-import-css`;
//! - [`map`] turns the styled tree into `addNode` commands with the report of what each node could not take, bounded
//!   at 100 commands; [`paint`] reads the paint of one element for it.
//!
//! The readers are written for this tree: there is no parser crate in it and no full parsing algorithm. They are
//! functions of the text. They open nothing, follow no URL (`src`, `href`, `url()`, `@import` and `<link>` are only
//! kept as text or listed), and run nothing. Their work is counted so that a test can show it is bounded by the size
//! of the text. Nothing in them reads a file, a clock or a thread, so they build for `wasm32-unknown-unknown` (L5).

/// The CSS side as its callers see it: the reader and the cascade of `html-import-css` and the style stage of
/// `html-import-style` (`value`, `vars`, `shorthand`, `defaults`, and the computed style) in one module.
pub mod css {
    pub use html_import_css::css::*;
    pub use html_import_style::*;
}
pub use html_import_css::tw;
pub use html_import_tree as html;
pub mod map;
pub mod paint;
