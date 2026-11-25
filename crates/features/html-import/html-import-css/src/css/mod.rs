//! The CSS reader: the text of `<style>` blocks and of the extra `css` argument, and the value of a `style`
//! attribute, read into rules and declarations (ADR-0024, `docs/import-html.md` sections 2.2 and 3.3). It is the
//! first stage of the CSS side of the importer: it reads, it does not cascade (the later stages do) and it maps
//! nothing to Nodes. This stage reads declarations; the rules and the at-rules come after it.
//!
//! - **Declarations**: `name: value` with `!important`; the name folded to lower case (a custom property keeps its
//!   case); comments dropped (a space in a value); strings and escapes kept whole; a NUL is the replacement
//!   character. A piece that is not a declaration is refused and listed (`invalid_declaration`, `empty_value`); that
//!   includes a string that ends at its line, as CSS says (the declaration is left out). A rule inside a block is
//!   skipped (`nested_rule_skipped`). `url()` in a value is kept as text and never followed.
//! - **Cut, and listed** with [`EntryKind::Cut`]: more than [`Limits::max_decls`] declarations
//!   (`declaration_limit`), a value over [`Limits::max_value_bytes`] (`value_too_long`; the name, the colon, the value
//!   and `!important` together are cut at the limit plus 256 bytes). Bytes are counted without
//!   the white space before a piece, and a run of white space and comments inside one is one byte: only text is
//!   too long.
//! - **Recovered**: text that ends inside a string, a comment, a bracket or a block (`unterminated`).
//!
//! The read is one pass: no recursion on the input, no look back, and a piece longer than its cap is read and not
//! stored. The work is counted in [`Report::work`].

mod decl;
mod scan;
pub mod selector;

pub use decl::{Bad, Decl};
pub use html_import_tree::{Entry, EntryKind, MAX_ENTRIES, MAX_REPORT_BYTES, Report};
pub use selector::{Combinator, Compound, Pseudo, Refusal, Selector, Simple};

use scan::{Cursor, End, Piece};

/// What the CSS reader is bounded by (ADR-0024: rules 5,000 and a value of 1,000 bytes; the other three numbers
/// are starting guesses of this stage, with nothing behind them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// The most rules kept, over all the texts. A rule is a block with at least one selector that is read and at
    /// least one declaration. The read stops at the next one. 5,000.
    pub max_rules: usize,
    /// The most declarations kept in one read (a style sheet, or one `style` attribute). Later ones are left out.
    /// 50,000.
    pub max_decls: usize,
    /// The most bytes of a value. A declaration with a longer one is left out. 1,000. The reader keeps a declaration
    /// only up to this plus 256 bytes in all (the name, the colon, the value and `!important`), so a name of 256 bytes
    /// or more makes a value of the limit too long: the 256 is the room for the name.
    pub max_value_bytes: usize,
    /// The most bytes of the selector text of a rule; a rule with more is left out. 4,096.
    pub max_selector_bytes: usize,
    /// How deep `@layer` blocks may nest; a deeper one is skipped. 16, as `variables-import`.
    pub max_depth: usize,
    /// The most selectors kept, over all the rules: `a,a,a,...` makes a selector for each two bytes. Later ones are
    /// left out. 5,000.
    pub max_selectors: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_rules: 5_000,
            max_decls: 50_000,
            max_value_bytes: 1_000,
            max_selector_bytes: 4_096,
            max_depth: 16,
            max_selectors: 5_000,
        }
    }
}

struct Reader<'r> {
    l: &'r Limits,
    report: &'r mut Report,
    decls: usize,
    base: usize,
}

/// Read the value of a `style` attribute: declarations only. The byte of a `report` entry is `at` (where the
/// element starts, the caller knows; the tree keeps no more) plus the byte within the value.
pub fn read_declarations(text: &str, at: usize, limits: &Limits, report: &mut Report) -> Vec<Decl> {
    let mut r = Reader {
        l: limits,
        report,
        decls: 0,
        base: at,
    };
    let mut c = Cursor::new(text);
    let out = r.decl_block(&mut c, false);
    r.report.work += c.work;
    out
}

impl Reader<'_> {
    fn refused(&mut self, code: &'static str, name: &str, message: impl Into<String>, at: usize) {
        let at = self.base + at;
        self.report
            .note(EntryKind::Refused, code, name, message.into(), at);
    }

    fn note(&mut self, kind: EntryKind, code: &'static str, message: &str, at: usize) {
        let at = self.base + at;
        self.report.note(kind, code, "", message.to_owned(), at);
    }

    fn skip(&mut self, c: &mut Cursor) {
        if !c.skip_block() {
            let at = c.at;
            self.note(
                EntryKind::Recovered,
                "unterminated",
                "the text ends inside a block",
                at,
            );
        }
    }

    /// The declarations of a block up to its `}` (`closed`), or of a whole `style` attribute.
    fn decl_block(&mut self, c: &mut Cursor, closed: bool) -> Vec<Decl> {
        let mut out = Vec::new();
        loop {
            let p = c.piece(self.l.max_value_bytes + 256);
            if p.unterminated && p.end == End::Eof {
                let msg = "the text ends inside a string, a comment or a bracket";
                self.note(EntryKind::Recovered, "unterminated", msg, p.at);
            }
            match p.end {
                End::Open => {
                    c.consume();
                    let name = p.text.trim_ascii();
                    self.refused(
                        "nested_rule_skipped",
                        name,
                        "a rule inside a rule is not read",
                        p.at,
                    );
                    self.skip(c);
                    continue;
                }
                End::Close if !closed => {
                    // A `}` with nothing open: what is before it is not a declaration.
                    c.consume();
                    let t = p.text.replace('\0', " ");
                    if !t.trim_ascii().is_empty() {
                        self.refused(
                            "invalid_declaration",
                            t.trim_ascii(),
                            "text before a stray }",
                            p.at,
                        );
                    }
                    continue;
                }
                _ => self.declaration(&p, &mut out),
            }
            match p.end {
                End::Semi => c.consume(),
                End::Close => {
                    c.consume();
                    return out;
                }
                _ => {
                    if closed && !p.unterminated {
                        self.note(
                            EntryKind::Recovered,
                            "unterminated",
                            "the text ends inside a block",
                            p.at,
                        );
                    }
                    return out;
                }
            }
        }
    }

    fn declaration(&mut self, p: &Piece, out: &mut Vec<Decl>) {
        // A comment is a space in a declaration.
        let text = p.text.replace('\0', " ");
        let t = text.trim_ascii();
        if t.is_empty() {
            return;
        }
        if p.bad_string {
            return self.refused("invalid_declaration", t, "a string ends at its line", p.at);
        }
        match decl::parse(t, p.too_long, self.l.max_value_bytes) {
            Ok(d) if self.decls >= self.l.max_decls => {
                if self.decls == self.l.max_decls {
                    self.decls += 1;
                    let msg = format!(
                        "more than {} declarations: the rest are left out",
                        self.l.max_decls
                    );
                    self.note(EntryKind::Cut, "declaration_limit", &msg, p.at);
                }
                drop(d);
            }
            Ok(d) => {
                self.decls += 1;
                out.push(d);
            }
            Err(Bad::Syntax) => self.refused("invalid_declaration", t, "not a declaration", p.at),
            Err(Bad::Empty) => self.refused("empty_value", t, "a declaration with no value", p.at),
            Err(Bad::TooLong) => {
                let name = t.split(':').next().unwrap_or("");
                let msg = format!("the value is over {} bytes", self.l.max_value_bytes);
                self.note_named(EntryKind::Cut, "value_too_long", name, msg, p.at);
            }
        }
    }

    fn note_named(
        &mut self,
        kind: EntryKind,
        code: &'static str,
        name: &str,
        msg: String,
        at: usize,
    ) {
        let at = self.base + at;
        self.report.note(kind, code, name, msg, at);
    }
}
