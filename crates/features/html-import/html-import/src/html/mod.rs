//! The tolerant tree builder: HTML text, a fragment or a whole document, to a tree of elements and a report
//! ([`parse`]). It reads this subset and nothing more (ADR-0024):
//!
//! - **Tags and attributes.** Names are folded to ASCII lower case and cut to [`MAX_NAME_BYTES`]. Values may be
//!   quoted, single-quoted, bare or missing; the first occurrence of a name decides, kept or cut for its length, and a repeat is counted; event attributes (`onclick` and
//!   the like) are dropped and counted. A `/` before the `>` means nothing except on a void element, as in a browser.
//! - **Text.** Line ends become `\n`, NUL becomes U+FFFD, and the character references of `text.rs` are read. Spaces
//!   are kept as written (collapsing them is for the mapping). Text around a dropped comment is one text.
//! - **Dropped, and counted once by kind.** Comments, the doctype, processing instructions and CDATA (as bogus
//!   comments, up to the next `>`), `<script>` with its content, event attributes.
//! - **Raw text.** `<style>` keeps its content as one text, as written, for the CSS reader; `<title>` and
//!   `<textarea>` keep theirs as one text with references read. All end at their own end tag in any case, and at
//!   nothing else, so a `</style>` inside a CSS string ends the style, as in a browser.
//! - **Void elements** have no content. **Unknown elements** are kept like any other.
//! - **Implied end tags**: the table of `rules.rs` (`p`, which a block, a heading, `li`, `dd` or `dt` also ends, and `li`, `dd`, `dt`, `tr`, `td`, `th`, the
//!   table sections, `option`, `optgroup` and a heading after a heading), each looking at most [`Limits::max_depth`] open
//!   elements down. There is no `html`, `head` or `body` made up, no adoption of formatting elements and no foster
//!   parenting of table content.
//! - **End tags.** One closes the nearest of the last [`Limits::max_depth`] open elements that has its name, and
//!   everything open inside it (reported as `misnested_end_tag`); with none it is dropped (`stray_end_tag`). What is
//!   still open at the end is closed there (`unclosed_at_end`).
//! - **Unfinished text.** A tag, comment or raw text that reaches the end of the text ends the read there; what
//!   came before it stays.
//! - **Bounds** ([`Limits`]): input bytes and elements refuse the text; depth, attributes, a value and a text cut,
//!   and the report says so. An element the depth limit does not make has its content in the element it is in,
//!   except a `<style>`, `<title>` or `<textarea>`, whose content is dropped. A size counts bytes as written, and
//!   a NUL is stored as three bytes. At most `max_attributes` names of an element are looked at, kept or cut.
//!
//! Nothing is evaluated or followed: attribute values are text. The work is counted in [`Report::work`], and every
//! rule looks back a bounded number of open elements, so it is linear in the size of the text.

mod build;
mod limits;
mod report;
mod rules;
mod tag;
mod text;

pub use limits::{Limits, ParseError};
pub use report::{Entry, EntryKind, MAX_ENTRIES, MAX_REPORT_BYTES, Report};

/// The most bytes of a tag or attribute name; a longer one is cut.
pub const MAX_NAME_BYTES: usize = 64;

/// An element of a [`Tree`], by its place in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(usize);

impl ElementId {
    /// The place, from 0 for the root, in the order the start tags came in the text.
    pub fn index(self) -> usize {
        self.0
    }
}

/// An attribute: the name in lower case, the value with its references read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: String,
    pub value: String,
}

/// What an element holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Text(String),
    Element(ElementId),
}

/// An HTML element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// The tag in lower case; empty for the root.
    pub tag: String,
    pub attrs: Vec<Attr>,
    pub children: Vec<Child>,
    /// The element it is in (the root's parent is the root).
    pub parent: ElementId,
    /// The byte of the text where its start tag begins.
    pub at: usize,
}

impl Element {
    /// The value of the attribute `name` (lower case), if the element has it.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }
}

/// The elements of a text, held flat: the root, which stands for the text itself and has the top-level elements
/// and texts as its children, then every element in the order of its start tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    elements: Vec<Element>,
}

impl Tree {
    pub fn root_id(&self) -> ElementId {
        ElementId(0)
    }

    pub fn root(&self) -> &Element {
        &self.elements[0]
    }

    /// The element `id` of this tree.
    pub fn element(&self, id: ElementId) -> &Element {
        &self.elements[id.0]
    }

    /// How many elements the tree has, the root not counted.
    pub fn len(&self) -> usize {
        self.elements.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The content of every `<style>`, in the order of the text, as written: the CSS for the next stage.
    pub fn style_texts(&self) -> impl Iterator<Item = &str> {
        self.elements
            .iter()
            .filter(|e| e.tag == "style")
            .flat_map(|e| {
                e.children.iter().filter_map(|c| match c {
                    Child::Text(t) => Some(t.as_str()),
                    Child::Element(_) => None,
                })
            })
    }
}

/// A read: the tree and what the builder had to say about the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub tree: Tree,
    pub report: Report,
}

/// Read `html` into a tree. Refused only for a reason of [`ParseError`]; malformed markup is repaired and the
/// report says where. Pure, deterministic, and linear in the length of `html`.
pub fn parse(html: &str, limits: &Limits) -> Result<Parsed, ParseError> {
    if html.len() > limits.max_input_bytes {
        return Err(ParseError::TooLarge {
            bytes: html.len(),
            max: limits.max_input_bytes,
        });
    }
    build::build(html, limits)
}
