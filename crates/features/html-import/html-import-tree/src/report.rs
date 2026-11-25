//! What the builder says about a text: the repairs it made and what a bound cut, each with the byte where it
//! happened, and a count of everything it dropped. Bounded as the report of `variables-import` is: a fixed number
//! of entries and bytes, a name and a message cut, hidden characters removed from anything taken from the text.

use std::collections::BTreeMap;

/// The most entries a report lists; the count of the rest is [`Report::omitted`].
pub const MAX_ENTRIES: usize = 200;

/// The most bytes of text the entries of a report hold in all (names, messages and codes).
pub const MAX_REPORT_BYTES: usize = 32 * 1024;

const MAX_NAME_CHARS: usize = 80;
const MAX_MESSAGE_CHARS: usize = 240;

/// What kind of line a report entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// The markup was repaired: an unfinished tag, an end tag with nothing to close, an element too deep.
    Recovered,
    /// A bound cut something: an attribute left out, a text cut.
    Cut,
    /// The CSS reader left something out because the importer does not read it: a selector, an at-rule, a
    /// declaration (`html-import-css`).
    Refused,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::Recovered => "recovered",
            EntryKind::Cut => "cut",
            EntryKind::Refused => "refused",
        }
    }
}

/// One line of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: EntryKind,
    /// A stable reason: `unterminated_tag`, `stray_end_tag`, `depth_cut`, `attribute_too_long`...
    pub code: &'static str,
    /// The tag or attribute it is about, cut and cleaned (empty when it is about none).
    pub name: String,
    pub message: String,
    /// The byte of the text where it happened.
    pub at: usize,
}

/// The report of one read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// The repairs and cuts, in the order of the text, at most [`MAX_ENTRIES`] and [`MAX_REPORT_BYTES`] bytes.
    pub entries: Vec<Entry>,
    /// How many entries there would have been past those bounds.
    pub omitted: usize,
    /// How often each code happened, entries or not. The things that are only dropped have a count and no entry:
    /// `comment`, `doctype`, `script`, `event_attribute`, `duplicate_attribute`.
    pub counts: BTreeMap<&'static str, usize>,
    /// The work the builder did, in steps: a step is one piece of text read, one frame of the open elements
    /// looked at or closed, one attribute name compared, or one byte looked at after an `&`. A test reads it to
    /// show that the work grows with the text and no faster. Nothing else needs it.
    pub work: usize,
    bytes: usize,
}

impl Report {
    /// Count `code` once and list it when there is room.
    pub fn note(
        &mut self,
        kind: EntryKind,
        code: &'static str,
        name: &str,
        message: String,
        at: usize,
    ) {
        *self.counts.entry(code).or_insert(0) += 1;
        let name = clean(name, MAX_NAME_CHARS);
        let message = clean(&message, MAX_MESSAGE_CHARS);
        let size = name.len() + message.len() + code.len();
        if self.entries.len() >= MAX_ENTRIES || self.bytes + size > MAX_REPORT_BYTES {
            self.omitted += 1;
            return;
        }
        self.bytes += size;
        self.entries.push(Entry {
            kind,
            code,
            name,
            message,
            at,
        });
    }

    /// Count `code` `n` times, with no entry.
    pub fn count(&mut self, code: &'static str, n: usize) {
        *self.counts.entry(code).or_insert(0) += n;
    }
}

/// `s` with the characters a reader should not be shown removed (controls, bidi and other format characters), cut
/// to `max` characters.
fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !hidden(*c)).take(max).collect()
}

fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{AD}' | '\u{34F}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}'
                | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}' | '\u{E0000}'..='\u{E007F}'
        )
}
