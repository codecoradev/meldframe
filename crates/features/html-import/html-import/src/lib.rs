//! HTML import feature module (E4.4, ADR-0024, `docs/import-html.md`): it reads HTML text that an agent sends and
//! turns it into Nodes. This crate is the facade: it names the stages under the paths the callers use and will hold
//! the mapping to Commands. The stages are crates of their own so that none passes the size ceiling:
//!
//! - [`html`] is `html-import-tree`, the tolerant, bounded tree builder;
//! - [`css`] is `html-import-css`, the CSS side (and `tw`, the Tailwind decoder, when it lands).
//!
//! The readers are written for this tree: there is no parser crate in it and no full parsing algorithm. They are
//! functions of the text. They open nothing, follow no URL (`src`, `href`, `url()`, `@import` and `<link>` are only
//! kept as text or listed), and run nothing. Their work is counted so that a test can show it is bounded by the size
//! of the text. Nothing in them reads a file, a clock or a thread, so they build for `wasm32-unknown-unknown` (L5).

pub use html_import_css::css;
pub use html_import_tree as html;
