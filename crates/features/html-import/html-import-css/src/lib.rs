//! HTML import, CSS side (E4.4, ADR-0024, `docs/import-html.md` sections 2.2 and 3.3): what the importer reads of
//! the CSS an agent sends, as `<style>` text, the extra `css` argument, `style` attributes and Tailwind classes.
//! [`css`] holds the reader and the cascade; `tw` decodes the Tailwind classes the export writes. Pure functions of the text, bounded, with the work counted; nothing is opened, fetched or run.

pub mod css;
pub mod tw;
