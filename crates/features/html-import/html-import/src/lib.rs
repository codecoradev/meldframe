//! HTML import feature module (E4.4, ADR-0024, `docs/import-html.md`). It reads HTML text that an agent sends and
//! turns it into Nodes; this slice holds the first stage, the tree builder in [`html`].
//!
//! The reader is a tolerant, bounded one over a documented subset of HTML, written for this crate: there is no
//! parser crate in the tree and no full HTML parsing algorithm. It is a function of the text. It opens nothing,
//! follows no URL (`src`, `href` and `<link>` are only kept as text), and runs nothing (`<script>` is dropped
//! unread, event attributes are dropped). Its work is counted so that a test can show it is bounded by the size
//! of the text. Nothing in it reads a file, a clock or a thread, so it builds for `wasm32-unknown-unknown` (L5).

pub mod html;
