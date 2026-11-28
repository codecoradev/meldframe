//! The tolerant tree builder: HTML text, a fragment or a whole document, to a tree of elements and a report
//! ([`parse`]). It reads this subset and nothing more (ADR-0024):
//!
//! - **Tags and attributes.** Names are folded to ASCII lower case and cut to [`MAX_NAME_BYTES`]. Values may be
//!   quoted, single-quoted, bare or missing; the first occurrence of a name decides, kept or cut for its length, and
//!   a repeat is counted; event attributes (`onclick` and the like) are dropped and counted. A `/` before the `>`
//!   means nothing except on a void element, as in a browser.
//! - **Text.** Line ends become `\n`, NUL becomes U+FFFD, and the character references of `text.rs` are read. Spaces
//!   are kept as written (collapsing them is for the mapping). Text around a dropped comment is one text.
//! - **Dropped, and counted once by kind.** Comments, the doctype, processing instructions and CDATA (as bogus
//!   comments, up to the next `>`), `<script>` with its content, event attributes.
//! - **Raw text.** `<style>` keeps its content as one text, as written, for the CSS reader; `<title>` and
//!   `<textarea>` keep theirs as one text with references read. All end at their own end tag in any case, and at
//!   nothing else, so a `</style>` inside a CSS string ends the style, as in a browser.
//! - **Void elements** have no content. **Unknown elements** are kept like any other.
//! - **Implied end tags**: the table of `rules.rs` (`p`, which a block, a heading, `li`, `dd` or `dt` also ends,
//!   and `li`, `dd`, `dt`, `tr`, `td`, `th`, the table sections, `option`, `optgroup` and a heading after a
//!   heading), each looking at most [`Limits::max_depth`] open elements down. There is no `html`, `head` or `body` made up, no adoption of formatting elements and no foster
//!   parenting of table content.
//! - **End tags.** One closes the nearest of the last [`Limits::max_depth`] open elements that has its name, and
//!   everything open inside it (reported as `misnested_end_tag`); with none it is dropped (`stray_end_tag`). What is
//!   still open at the end is closed there (`unclosed_at_end`).
//! - **Unfinished text.** A tag, comment or raw text that reaches the end of the text ends the read there; what
//!   came before it stays.
//! - **Bounds** ([`Limits`]): input bytes and elements refuse the text; depth, attributes, a value and a text cut,
//!   and the report says so. An element the depth limit does not make has its content in the element it is in,
//!   except a `<style>`, `<title>` or `<textarea>`, whose content is dropped. A value limit counts the bytes as
//!   written, a text limit the bytes as stored (a NUL is three, a reference is what it stands for). At most `max_attributes` names of an element are looked at, kept or cut.
//!
//! Nothing is evaluated or followed: attribute values are text. The work is counted in [`Report::work`], and every
//! rule looks back a bounded number of open elements, so it is linear in the size of the text.

mod build;
mod limits;
mod report;
mod rules;
mod tag;
mod text;
mod tree;

pub use build::parse;
pub use limits::{Limits, ParseError};
pub use report::{Entry, EntryKind, MAX_ENTRIES, MAX_REPORT_BYTES, Report};
pub use tree::{Attr, Child, Element, ElementId, MAX_NAME_BYTES, Parsed, Tree};
