//! HTML import, CSS side (E4.4, ADR-0024, `docs/import-html.md` sections 2.2 and 3.3): the reader of the CSS an agent sends (`<style>` text, the extra `css` argument and `style` attributes), the cascade over the element tree and the decoder of the Tailwind classes the export writes, as pure functions of the text that are bounded and count their work.
//!
//! - [`css`] holds the reader, the selectors and the cascade;
//! - `tw` decodes the Tailwind classes, the inverse of the export's.
//!
//! Nothing is opened, fetched or run.

pub mod css;
pub mod tw;
