//! The CSS reader: the text of `<style>` blocks and of the extra `css` argument, and the value of a `style`
//! attribute, read into rules and declarations (ADR-0024, `docs/import-html.md` sections 2.2 and 3.3). It is the
//! first stage of the CSS side of the importer: it reads, it does not cascade (the later stages do) and it maps
//! nothing to Nodes.
//!
//! - **Read.** Style rules with the simple selectors of [`selector`], declarations with `!important`, comments
//!   dropped, strings and escapes kept whole, `@layer` transparent (a `@layer` block is read as if it were not
//!   there, and a `@layer` statement is ignored), and the `<!--` and `-->` of old style sheets ignored at the top.
//!   A comment is nothing inside a selector (`.a/**/.b` is `.a.b`) and a space in a value. A NUL is the replacement
//!   character.
//! - **Refused, and listed in the report** with [`EntryKind::Refused`]: a selector that is not simple
//!   (`selector_pseudo_class`, `selector_pseudo_element`, `selector_attribute`, `selector_sibling`,
//!   `selector_too_complex`, `selector_invalid`); an at-rule (`@media`, `@supports`, `@container`, `@font-face`,
//!   `@keyframes`, `@property` and any other: `at_rule_skipped`; `@import`: `import_skipped`, never followed); a
//!   rule nested in a rule (`nested_rule_skipped`); a piece that is not a declaration (`invalid_declaration`,
//!   `empty_value`), which includes a string that ends at its line, as CSS says (the declaration, or the whole rule
//!   when it is in the selector, is left out); text that is not a rule or an at-rule (`invalid_statement`), also
//!   when it comes before a `}`, also the one that closes a `@layer` block. `url()` in a value is kept as text and never followed. A close
//!   brace with nothing open is counted (`stray_close_brace`).
//! - **A selector list is read selector by selector.** A selector that is refused or invalid is left out alone and
//!   the others of its list stay. CSS drops the whole rule for an invalid selector, so this is more tolerant, on
//!   purpose: a state selector is never true in a static read, and the rest of the list is what the author meant.
//!   A rule with no selector left is skipped with its block.
//! - **Cut, and listed** with [`EntryKind::Cut`]: more than [`Limits::max_rules`] rules (the read stops there,
//!   `rule_limit`), more than [`Limits::max_selectors`] selectors (`selector_limit`, once), more than
//!   [`Limits::max_decls`] declarations (`declaration_limit`), a value over [`Limits::max_value_bytes`]
//!   (`value_too_long`; the name, the colon, the value and `!important` together are cut at the limit plus 256
//!   bytes), a selector list over [`Limits::max_selector_bytes`] (`selector_too_long`), layers nested
//!   deeper than [`Limits::max_depth`] (`depth_cut`). Bytes are counted without the white space before a piece, and
//!   a long run of white space inside one is one space: only text is too long.
//! - **Recovered**: text that ends inside a string, a comment, a bracket, a block or a `@layer` block, or after
//!   text with no end (`unterminated`).
//!
//! Several texts (the `<style>` blocks in document order, then the extra CSS) are one style sheet: rules are
//! numbered across them and the byte of a report entry counts them one after the other. The read is one pass: no
//! recursion on the input (a `@layer` nests at most [`Limits::max_depth`]), no look back, and a piece longer than
//! its cap is read and not stored. The work is counted in [`Report::work`].

mod cascade;
mod decl;
mod matching;
mod scan;
pub mod selector;
pub mod value;
pub mod vars;

pub use cascade::{Candidate, Cascade, Matched, Origin, cascade};
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
    /// left out. 10,000 (two for each of the most rules, so that the rule limit can be reached).
    pub max_selectors: usize,
    /// The most steps the cascade takes in all (a step is a selector looked at, a compound tested or a declaration
    /// made a candidate); the elements past it get no styles and the report says so. 2,000,000.
    pub max_work: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_rules: 5_000,
            max_decls: 50_000,
            max_value_bytes: 1_000,
            max_selector_bytes: 4_096,
            max_depth: 16,
            max_selectors: 10_000,
            max_work: 2_000_000,
        }
    }
}

/// A style rule: its selectors (at least one) and its declarations, in the order written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub decls: Vec<Decl>,
    /// The byte of the text where the rule starts.
    pub at: usize,
}

/// The rules of the texts read, in source order: the index of a rule is its place in the cascade.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

struct Reader<'r> {
    l: &'r Limits,
    report: &'r mut Report,
    rules: Vec<Rule>,
    decls: usize,
    selectors: usize,
    selectors_cut: bool,
    base: usize,
    stop: bool,
}

/// Read `texts` as one style sheet. Never fails: what is not read is in `report`.
pub fn read<'a>(
    texts: impl IntoIterator<Item = &'a str>,
    limits: &Limits,
    report: &mut Report,
) -> Stylesheet {
    let mut r = Reader {
        l: limits,
        report,
        rules: Vec::new(),
        decls: 0,
        selectors: 0,
        selectors_cut: false,
        base: 0,
        stop: false,
    };
    for text in texts {
        let mut c = Cursor::new(text);
        r.rules(&mut c, 0);
        r.report.work += c.work;
        r.base += c.len();
    }
    Stylesheet { rules: r.rules }
}

/// Read the value of a `style` attribute: declarations only. The byte of a `report` entry is `at` (where the
/// element starts, the caller knows; the tree keeps no more) plus the byte within the value.
pub fn read_declarations(text: &str, at: usize, limits: &Limits, report: &mut Report) -> Vec<Decl> {
    let mut r = Reader {
        l: limits,
        report,
        rules: Vec::new(),
        decls: 0,
        selectors: 0,
        selectors_cut: false,
        base: at,
        stop: false,
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

    /// The statements of a text, or of a `@layer` block, up to its `}`.
    fn rules(&mut self, c: &mut Cursor, depth: usize) {
        while !self.stop {
            let mut p = c.piece(self.l.max_selector_bytes);
            if depth == 0 {
                // `<!--` and `-->` between the rules of an old style sheet mean nothing.
                let t = strip_marks(&p.text);
                if t.len() != p.text.len() {
                    p.text = t.to_owned();
                }
            }
            let blank = p.text.trim_ascii().is_empty();
            let cut_off = p.end == End::Eof && (!blank || depth > 0);
            if p.unterminated || cut_off {
                let msg =
                    "the text ends inside a string, a comment, a bracket, a block or a statement";
                self.note(EntryKind::Recovered, "unterminated", msg, p.at);
            }
            match p.end {
                End::Eof => return,
                End::Close => {
                    c.consume();
                    // Text before a `}` is not a rule: it is listed, whether the `}` closes a layer or is stray.
                    if !blank {
                        self.statement(&p);
                    }
                    if depth > 0 {
                        return;
                    }
                    self.report.count("stray_close_brace", 1);
                }
                End::Semi => {
                    c.consume();
                    self.statement(&p);
                }
                End::Open => {
                    c.consume();
                    self.open(c, &p, depth);
                }
            }
        }
    }

    /// The name of the at-rule in `text` (`@media (x)` is `media`), lower case.
    fn at_name(text: &str) -> String {
        text.strip_prefix('@')
            .unwrap_or("")
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect::<String>()
            .to_ascii_lowercase()
    }

    fn statement(&mut self, p: &Piece) {
        let text = p.text.replace('\0', " ");
        let t = text.trim_ascii();
        if t.is_empty() {
            return;
        }
        if t.starts_with('@') {
            match Self::at_name(t).as_str() {
                "layer" => {}
                "import" => self.refused(
                    "import_skipped",
                    t,
                    "@import is never followed: send the text of that file as css",
                    p.at,
                ),
                n => self.refused("at_rule_skipped", t, format!("@{n} is not read"), p.at),
            }
        } else {
            self.refused("invalid_statement", t, "not a rule or an at-rule", p.at);
        }
    }

    /// A block opens after the prelude `p`.
    fn open(&mut self, c: &mut Cursor, p: &Piece, depth: usize) {
        let at_text = p.text.replace('\0', " ");
        let pre = p.text.trim_ascii();
        if pre.starts_with('@') {
            let pre = at_text.trim_ascii();
            let name = Self::at_name(pre);
            if name == "layer" && depth < self.l.max_depth {
                return self.rules(c, depth + 1);
            }
            if name == "layer" {
                let msg = "layers nest too deep; the block is skipped";
                self.note(EntryKind::Cut, "depth_cut", msg, p.at);
            } else {
                self.refused("at_rule_skipped", pre, format!("@{name} is not read"), p.at);
            }
            return self.skip(c);
        }
        if p.bad_string {
            self.refused("selector_invalid", pre, "a string ends at its line", p.at);
            return self.skip(c);
        }
        if p.too_long {
            self.note(
                EntryKind::Cut,
                "selector_too_long",
                "the selector is over the limit",
                p.at,
            );
            return self.skip(c);
        }
        let mut selectors = Vec::new();
        for text in selector::split_list(pre) {
            if self.selectors >= self.l.max_selectors {
                if !self.selectors_cut {
                    self.selectors_cut = true;
                    let msg = format!(
                        "more than {} selectors: the rest are left out",
                        self.l.max_selectors
                    );
                    self.note(EntryKind::Cut, "selector_limit", &msg, p.at);
                }
                break;
            }
            if text.trim_ascii().is_empty() {
                self.refused(
                    "selector_invalid",
                    "",
                    "a selector list has an empty selector",
                    p.at,
                );
                continue;
            }
            match selector::parse(text, &mut self.report.work) {
                Ok(s) => {
                    self.selectors += 1;
                    selectors.push(s);
                }
                Err(e) => self.refused(e.code, text.trim_ascii(), e.why, p.at),
            }
        }
        if selectors.is_empty() {
            return self.skip(c);
        }
        let decls = self.decl_block(c, true);
        if decls.is_empty() {
            return;
        }
        if self.rules.len() >= self.l.max_rules {
            let msg = format!("more than {} rules: the rest is not read", self.l.max_rules);
            self.note(EntryKind::Cut, "rule_limit", &msg, p.at);
            self.stop = true;
            return;
        }
        self.rules.push(Rule {
            selectors,
            decls,
            at: self.base + p.at,
        });
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

/// `s` without the `<!--` and `-->` (and the white space and comments around them) that an old style sheet has
/// between its rules.
fn strip_marks(mut s: &str) -> &str {
    loop {
        let t = s.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '\0');
        match t.strip_prefix("<!--").or_else(|| t.strip_prefix("-->")) {
            Some(rest) => s = rest,
            None => return t,
        }
    }
}
