//! A scanner for the custom properties of a style sheet. It is not a CSS engine: it cuts the text into blocks and
//! declarations, follows nothing (`@import` and `url()` are never opened) and keeps only the custom properties that
//! sit where a variable lives: in `@theme`, or in a rule on the root that picks a mode. Everything else is counted and
//! reported, never run. One pass, bounded depth and count, no recursion on the input's own nesting beyond the cap.

use crate::text::short;
use std::collections::HashMap;

/// How deep blocks may nest before the rest of the block is skipped.
pub const MAX_DEPTH: usize = 16;
/// The most custom properties kept.
pub const MAX_DECLS: usize = 20_000;
/// The most left-out rules and at-rules kept; the rest are only counted in [`Scan::left_omitted`].
pub const MAX_LEFT: usize = 1_000;

/// Where a declaration sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// In `@theme` (any of its forms).
    Theme,
    /// In a rule on the root, `:root` and the selectors that pick a mode.
    Root,
}

/// A custom property kept from the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    /// The property name as written, with the leading `--`.
    pub name: String,
    pub value: String,
    pub section: Section,
    /// The rule sets it for the default (light) mode, for the dark mode, or both.
    pub light: bool,
    pub dark: bool,
    /// The selector or at-rule the declaration came from, for the report: an index into [`Scan::sources`].
    pub source: u32,
}

/// Something the scanner left out.
#[derive(Debug, Clone, PartialEq)]
pub struct Left {
    pub code: &'static str,
    pub what: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Scan {
    pub decls: Vec<Decl>,
    /// The distinct labels of the rules the declarations came from, each cut to 80 characters.
    pub sources: Vec<String>,
    pub left: Vec<Left>,
    /// Rules and at-rules left out beyond [`MAX_LEFT`].
    pub left_omitted: usize,
    /// Properties other than custom properties, which are never variables.
    pub other_properties: usize,
    /// More than [`MAX_DECLS`] custom properties.
    pub too_many: bool,
    /// The text ended inside a string, a comment or a block.
    pub unterminated: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct Ctx {
    section: Option<Section>,
    light: bool,
    dark: bool,
}

struct Reader<'a> {
    b: &'a [u8],
    text: &'a str,
    at: usize,
    out: Scan,
    seen: HashMap<String, u32>,
}

/// What a media query says about the mode.
enum Media {
    Pick(Pick),
    /// It names `prefers-color-scheme` in a form that is not read: negated, compound, or another value.
    Unread,
    /// It is about something else.
    Other,
}

/// `(prefers-color-scheme: dark)` or `light`, alone or after `screen and` or `all and`, in any spacing and case.
fn media(args: &str) -> Media {
    // White space is one space, and none is needed inside the parentheses or around the colon; `screenand(` stays
    // what it is, which is not `screen and (`.
    let q = args
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let q = q
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" :", ":")
        .replace(": ", ":");
    let q = q
        .strip_prefix("screen and ")
        .or_else(|| q.strip_prefix("all and "))
        .unwrap_or(&q);
    match q {
        "(prefers-color-scheme:dark)" => Media::Pick(Pick::Dark),
        "(prefers-color-scheme:light)" => Media::Pick(Pick::Light),
        q if q.contains("prefers-color-scheme") => Media::Unread,
        _ => Media::Other,
    }
}

/// The kind of mode a single selector of a rule picks, if it picks one.
#[derive(Debug, PartialEq)]
enum Pick {
    /// The root, which is the default mode (and the dark mode inside a dark query).
    Light,
    /// A rule that names the light mode on purpose: `.light`, `[data-theme=light]`.
    ExplicitLight,
    Dark,
}

/// `[data-theme=dark]`, `[data-theme="dark"]`, `[data-mode='dark']`: the value an attribute selector tests.
fn attr_value(s: &str) -> Option<&str> {
    let inner = s.strip_prefix('[')?.strip_suffix(']')?;
    let (name, v) = inner.split_once('=')?;
    if !name.trim().starts_with("data-") {
        return None;
    }
    Some(v.trim().trim_matches(['"', '\'']))
}

fn pick(selector: &str) -> Option<Pick> {
    let s = selector.trim();
    if matches!(
        s,
        ":root" | "html" | ":host" | "body" | ":root:not(.dark)" | "html:not(.dark)"
    ) {
        return Some(Pick::Light);
    }
    let rest = [":root", "html", "body"]
        .iter()
        .find_map(|r| s.strip_prefix(r))
        .unwrap_or(s);
    let rest = rest.strip_suffix(" *").unwrap_or(rest).trim();
    let word = match rest {
        ".dark" => "dark",
        ".light" => "light",
        _ => attr_value(rest)?,
    };
    match word {
        "dark" => Some(Pick::Dark),
        "light" => Some(Pick::ExplicitLight),
        _ => None,
    }
}

/// The text of a selector list cut at the commas that are not inside brackets or parentheses.
fn selectors(prelude: &str) -> Vec<&str> {
    let (mut depth, mut start, mut out) = (0usize, 0, Vec::new());
    for (i, c) in prelude.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(&prelude[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&prelude[start..]);
    out
}

impl<'a> Reader<'a> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.at).copied()
    }

    /// Skip a comment if one starts here; false when none does.
    fn comment(&mut self) -> bool {
        if self.b[self.at..].starts_with(b"/*") {
            match self.text[self.at + 2..].find("*/") {
                Some(i) => self.at += 2 + i + 2,
                None => {
                    self.at = self.b.len();
                    self.out.unterminated = true;
                }
            }
            return true;
        }
        false
    }

    /// Read up to the next `;`, `{` or `}` outside strings, comments and brackets. Returns the text read (comments
    /// removed) and the byte that ended it, which is not consumed (`None` at the end of the text).
    fn prelude(&mut self) -> (String, Option<u8>) {
        let mut out = String::new();
        let mut depth = 0usize;
        while let Some(c) = self.peek() {
            if self.comment() {
                out.push(' ');
                continue;
            }
            match c {
                b'"' | b'\'' => {
                    let start = self.at;
                    self.at += 1;
                    loop {
                        match self.peek() {
                            None => {
                                self.out.unterminated = true;
                                break;
                            }
                            Some(b'\\') => {
                                self.at += 1;
                                self.at += self
                                    .text
                                    .get(self.at..)
                                    .and_then(|t| t.chars().next())
                                    .map_or(0, char::len_utf8);
                            }
                            Some(q) if q == c => {
                                self.at += 1;
                                break;
                            }
                            Some(_) => self.at += 1,
                        }
                    }
                    let end = self.at.min(self.b.len());
                    out.push_str(&self.text[start..end]);
                }
                b'(' | b'[' => {
                    depth += 1;
                    out.push(c as char);
                    self.at += 1;
                }
                b')' | b']' => {
                    depth = depth.saturating_sub(1);
                    out.push(c as char);
                    self.at += 1;
                }
                b';' | b'{' | b'}' if depth == 0 => return (out, Some(c)),
                _ => {
                    // Copy a whole UTF-8 character.
                    let ch = self.text[self.at..].chars().next().unwrap_or(' ');
                    out.push(ch);
                    self.at += ch.len_utf8();
                }
            }
        }
        (out, None)
    }

    /// The index of a label in `sources`, added when it is new. A label is at most 80 characters.
    fn intern(&mut self, label: &str) -> u32 {
        if let Some(&i) = self.seen.get(label) {
            return i;
        }
        let i = self.out.sources.len() as u32;
        self.out.sources.push(label.to_string());
        self.seen.insert(label.to_string(), i);
        i
    }

    fn leave(&mut self, code: &'static str, what: &str, message: impl Into<String>) {
        if self.out.left.len() >= MAX_LEFT {
            self.out.left_omitted += 1;
            return;
        }
        self.out.left.push(Left {
            code,
            what: short(what),
            message: message.into(),
        });
    }

    /// The statements of one block (or of the whole text) until its `}`.
    fn block(&mut self, ctx: Ctx, depth: usize, source: &str) {
        loop {
            let (pre, end) = self.prelude();
            let pre = pre.trim();
            match end {
                None => {
                    if !pre.is_empty() {
                        self.out.unterminated = true;
                    }
                    return;
                }
                Some(b'}') => {
                    self.at += 1;
                    if !pre.is_empty() {
                        self.declaration(pre, ctx, source);
                    }
                    if depth > 0 {
                        return;
                    }
                }
                Some(b';') => {
                    self.at += 1;
                    if !pre.is_empty() {
                        self.statement(pre, ctx, source);
                    }
                }
                Some(_) => {
                    self.at += 1;
                    self.open(pre, ctx, depth);
                }
            }
        }
    }

    /// A statement ended by `;`: an at-rule without a block, or a declaration.
    fn statement(&mut self, pre: &str, ctx: Ctx, source: &str) {
        if let Some(rest) = pre.strip_prefix('@') {
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '(' || c == '"')
                .next()
                .unwrap_or("");
            if name.eq_ignore_ascii_case("layer") {
                return;
            }
            if name.eq_ignore_ascii_case("import") {
                self.leave(
                    "import_skipped",
                    pre,
                    "@import is never followed: send the text of that file as well",
                );
            } else {
                let name = short(name);
                self.leave("at_rule_skipped", pre, format!("@{name} is not read"));
            }
            return;
        }
        self.declaration(pre, ctx, source);
    }

    fn declaration(&mut self, pre: &str, ctx: Ctx, source: &str) {
        let Some((name, value)) = pre.split_once(':') else {
            return;
        };
        let name = name.trim();
        if !name.starts_with("--") {
            self.out.other_properties += 1;
            return;
        }
        let Some(section) = ctx.section else {
            return;
        };
        if self.out.decls.len() >= MAX_DECLS {
            self.out.too_many = true;
            return;
        }
        let value = value.trim();
        let value = value
            .strip_suffix("!important")
            .map_or(value, str::trim_end);
        let source = self.intern(source);
        self.out.decls.push(Decl {
            name: name.to_string(),
            value: value.to_string(),
            section,
            light: ctx.light,
            dark: ctx.dark,
            source,
        });
    }

    /// A block opens after `pre`: pick what it is, read it with the right context, or skip it whole.
    fn open(&mut self, pre: &str, ctx: Ctx, depth: usize) {
        if depth + 1 > MAX_DEPTH {
            self.leave(
                "too_deep",
                pre,
                format!("blocks nest over {MAX_DEPTH} levels; the block is skipped"),
            );
            self.skip_block();
            return;
        }
        let label = short(pre);
        if let Some(rest) = pre.strip_prefix('@') {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .unwrap_or(rest.len());
            let (name, args) = rest.split_at(end);
            match name.to_ascii_lowercase().as_str() {
                "theme" if ctx.section.is_none() => {
                    let c = Ctx {
                        section: Some(Section::Theme),
                        light: true,
                        dark: false,
                    };
                    self.block(c, depth + 1, "@theme");
                }
                "layer" => self.block(ctx, depth + 1, "@layer"),
                "media" => match media(args) {
                    Media::Pick(Pick::Dark) => {
                        let c = Ctx {
                            dark: true,
                            light: false,
                            ..ctx
                        };
                        self.block(c, depth + 1, &label);
                    }
                    Media::Pick(Pick::Light | Pick::ExplicitLight) => {
                        self.block(ctx, depth + 1, &label)
                    }
                    Media::Unread => {
                        let why = "a media query on prefers-color-scheme that is negated, compound or not dark or light is not read";
                        self.leave("media_query_skipped", pre, why);
                        self.skip_block();
                    }
                    Media::Other => {
                        self.leave("at_rule_skipped", pre, "@media is not read");
                        self.skip_block();
                    }
                },
                _ => {
                    self.leave("at_rule_skipped", pre, format!("@{name} is not read"));
                    self.skip_block();
                }
            }
            return;
        }
        // A style rule: a list of selectors.
        let (mut light, mut dark, mut other, mut explicit) = (false, false, false, false);
        for s in selectors(pre) {
            match pick(s) {
                Some(Pick::Light) => light = true,
                Some(Pick::ExplicitLight) => {
                    light = true;
                    explicit = true;
                }
                Some(Pick::Dark) => dark = true,
                None => other = true,
            }
        }
        // A media query that said dark already set `ctx.dark`: a root rule inside it is the dark mode.
        let in_dark_media = ctx.dark && !ctx.light && ctx.section.is_none();
        if explicit && in_dark_media && !other && ctx.section.is_none() {
            let why = "a rule for the light mode inside a dark query is neither: it is not read";
            self.leave("light_in_dark_media", pre, why);
            self.skip_block();
        } else if (light || dark) && !other && ctx.section.is_none() {
            let c = Ctx {
                section: Some(Section::Root),
                light: light && !in_dark_media,
                dark: dark || in_dark_media,
            };
            self.block(c, depth + 1, &label);
        } else {
            self.leave(
                "selector_skipped",
                pre,
                "the rule is not on the root and does not pick a mode, so its custom properties are not variables",
            );
            self.skip_block();
        }
    }

    /// Read past the block that just opened, keeping nothing. A loop, not a recursion, so that nesting cannot grow
    /// the stack.
    fn skip_block(&mut self) {
        let mut open = 1usize;
        while open > 0 {
            match self.prelude().1 {
                None => return,
                Some(b'{') => open += 1,
                Some(b'}') => open -= 1,
                Some(_) => {}
            }
            self.at += 1;
        }
    }
}

/// Scan `text` for the custom properties of a theme.
pub fn scan(text: &str) -> Scan {
    let mut r = Reader {
        b: text.as_bytes(),
        text,
        at: 0,
        out: Scan::default(),
        seen: HashMap::new(),
    };
    let start = Ctx::default();
    r.block(start, 0, "");
    r.out
}
