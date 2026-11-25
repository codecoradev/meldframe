//! HTML import feature module (E4.4, ADR-0024, `docs/import-html.md`). It reads HTML text that an agent sends and
//! turns it into Nodes. The stages so far: the tree builder in [`html`] and the CSS reader in [`css`].
//!
//! The readers are tolerant, bounded ones over a documented subset of HTML and CSS, written for this crate: there
//! is no parser crate in the tree and no full parsing algorithm. They are functions of the text. They open
//! nothing, follow no URL (`src`, `href`, `url()`, `@import` and `<link>` are only kept as text or listed), and run
//! nothing (`<script>` is dropped unread, event attributes are dropped). Their work is counted so that a test can
//! show it is bounded by the size of the text. Nothing in them reads a file, a clock or a thread, so they build for
//! `wasm32-unknown-unknown` (L5).

pub mod css;
pub mod html;
pub mod tw;
