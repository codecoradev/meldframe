//! Selectors: the simple ones of `docs/import-html.md` section 3.3 and nothing else. A selector is compounds joined
//! by descendant (space) and child (`>`) combinators; a compound is type, `.class`, `#id`, `[attr]`, `[attr=value]`,
//! `*` and `:root`, and the last compound may end in `::before` or `::after` (the export's border is
//! `[data-node="x"]::after`). Anything else is refused with a reason, as a whole selector: a list keeps its other
//! selectors. Identifiers and strings are read with their CSS escapes. Specificity is worked out here.

/// How two compounds are joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
}

/// One test of an element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Simple {
    /// A type selector, in lower case.
    Tag(String),
    Class(String),
    Id(String),
    /// `[name]` (value `None`) or `[name=value]`; the name in lower case.
    Attr(String, Option<String>),
    /// `:root`, the `html` element.
    Root,
}

/// The generated box a selector styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pseudo {
    Before,
    After,
}

/// Simple selectors with no space between them; none at all is `*`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Compound {
    pub simples: Vec<Simple>,
}

/// A selector: `compounds[i]` and `compounds[i + 1]` are joined by `combinators[i]`; the last compound is the one
/// that picks the element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub compounds: Vec<Compound>,
    pub combinators: Vec<Combinator>,
    pub pseudo: Option<Pseudo>,
    /// Ids, then classes with attributes and `:root`, then types and the pseudo-element, one byte each (a count
    /// over 255 stays 255), as one number that orders by the three in turn.
    pub specificity: u32,
}

/// Why a selector is not read: a stable code (a report code) and a short reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub why: String,
}

fn no(code: &'static str, why: impl Into<String>) -> Refusal {
    Refusal {
        code,
        why: why.into(),
    }
}

/// The most compounds in a selector and simples in a compound.
pub const MAX_COMPOUNDS: usize = 16;
pub const MAX_SIMPLES: usize = 16;

/// The text of a selector list cut at the commas outside strings, brackets and parentheses. Linear.
pub fn split_list(prelude: &str) -> Vec<&str> {
    let (mut depth, mut start, mut quote, mut out) = (0usize, 0, None::<u8>, Vec::new());
    let b = prelude.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match (quote, b[i]) {
            (Some(_), b'\\') => i += 1,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, b'\\') => i += 1,
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'(' | b'[') => depth += 1,
            (None, b')' | b']') => depth = depth.saturating_sub(1),
            (None, b',') if depth == 0 => {
                out.push(&prelude[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&prelude[start.min(prelude.len())..]);
    out
}

struct P<'a> {
    s: &'a str,
    i: usize,
    work: &'a mut usize,
    /// A backslash was followed by a line end, which is not an escape: the selector is invalid.
    bad: bool,
}

fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()
}

impl P<'_> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        *self.work += 1;
        Some(c)
    }

    /// Past white space and comments (the reader leaves a comment as a NUL). True when some of it was white space.
    fn space(&mut self) -> bool {
        let mut seen = false;
        while let Some(c) = self
            .peek()
            .filter(|c| c.is_ascii_whitespace() || *c == '\0')
        {
            seen |= c != '\0';
            self.bump();
        }
        seen
    }

    /// An escape after its `\`: one to six hex digits and the space that may end them, else the next character.
    fn escape(&mut self) -> Option<char> {
        let first = self.peek()?;
        // A backslash before a line end (CR, LF, FF) is not an escape.
        if matches!(first, '\n' | '\r' | '\u{c}') {
            return None;
        }
        if !first.is_ascii_hexdigit() {
            self.bump();
            return Some(first);
        }
        let mut v = 0u32;
        for _ in 0..6 {
            match self.peek().and_then(|c| c.to_digit(16)) {
                Some(d) => {
                    v = v * 16 + d;
                    self.bump();
                }
                None => break,
            }
        }
        if self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            self.bump();
        }
        Some(
            char::from_u32(v)
                .filter(|c| *c != '\0')
                .unwrap_or('\u{fffd}'),
        )
    }

    /// A name: letters, digits, `-`, `_`, non-ASCII characters and escapes. Empty when none starts here.
    fn name(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == '\\' {
                self.bump();
                match self.escape() {
                    Some(e) => out.push(e),
                    None => {
                        self.bad = true;
                        break;
                    }
                }
            } else if name_char(c) {
                self.bump();
                out.push(c);
            } else {
                break;
            }
        }
        out
    }

    /// An identifier, which is what a type, a class, an id and an attribute name are: a name that does not start
    /// with a digit, or a hyphen and a digit, and is not empty.
    fn ident(&mut self) -> Result<String, Refusal> {
        let mut chars = self.s[self.i..].chars();
        let (a, b) = (chars.next(), chars.next());
        let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
        if digit(a)
            || (a == Some('-') && (digit(b) || b.is_none_or(|c| !name_char(c) && c != '\\')))
        {
            return Err(no(
                "selector_invalid",
                "a name starts with a digit or a lone hyphen",
            ));
        }
        let n = self.name();
        if n.is_empty() || self.bad {
            return Err(no(
                "selector_invalid",
                "a name is empty or has an escape that is not one",
            ));
        }
        Ok(n)
    }

    fn string(&mut self, quote: char) -> Result<String, Refusal> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(no("selector_invalid", "a string is not closed")),
                Some(c) if c == quote => return Ok(out),
                Some('\n' | '\r' | '\u{c}') => {
                    return Err(no("selector_invalid", "a string ends at its line"));
                }
                Some('\\') => match self.peek() {
                    // A backslash and a line end (CRLF is one) join the lines.
                    Some('\r') => {
                        self.bump();
                        if self.peek() == Some('\n') {
                            self.bump();
                        }
                    }
                    Some('\n' | '\u{c}') => {
                        self.bump();
                    }
                    _ => {
                        if let Some(e) = self.escape() {
                            out.push(e);
                        }
                    }
                },
                Some(c) => out.push(c),
            }
        }
    }

    fn attr(&mut self) -> Result<Simple, Refusal> {
        self.space();
        let name = self.ident()?.to_ascii_lowercase();
        self.space();
        match self.bump() {
            Some(']') => Ok(Simple::Attr(name, None)),
            Some('=') => {
                self.space();
                let value = match self.peek() {
                    Some(q @ ('"' | '\'')) => self.string(q)?,
                    _ => {
                        let v = self.name();
                        if v.is_empty() {
                            return Err(no(
                                "selector_invalid",
                                "an attribute selector has no value",
                            ));
                        }
                        v
                    }
                };
                self.space();
                match self.bump() {
                    Some(']') => Ok(Simple::Attr(name, Some(value))),
                    _ => Err(no(
                        "selector_attribute",
                        "an attribute flag or a value that is not one word or one string is not read",
                    )),
                }
            }
            Some('~' | '|' | '^' | '$' | '*') => Err(no(
                "selector_attribute",
                "only [attr] and [attr=value] are read",
            )),
            _ => Err(no(
                "selector_invalid",
                "an attribute selector is not closed",
            )),
        }
    }

    fn pseudo(&mut self) -> Result<PseudoRead, Refusal> {
        let element = self.peek() == Some(':');
        if element {
            self.bump();
        }
        let name = self.name().to_ascii_lowercase();
        let functional = self.peek() == Some('(');
        match (element, name.as_str(), functional) {
            (false, "root", false) => Ok(PseudoRead::Simple(Simple::Root)),
            // The single colon is the old way to write these two.
            (_, "before", false) => Ok(PseudoRead::Element(Pseudo::Before)),
            (_, "after", false) => Ok(PseudoRead::Element(Pseudo::After)),
            (true, _, _) => Err(no(
                "selector_pseudo_element",
                format!("::{name} is not read (only ::before and ::after)"),
            )),
            _ => Err(no(
                "selector_pseudo_class",
                format!(":{name} has no static meaning here (only :root is read)"),
            )),
        }
    }

    fn compound(&mut self) -> Result<(Compound, Option<Pseudo>), Refusal> {
        let mut c = Compound::default();
        let mut pseudo = None;
        let mut first = true;
        loop {
            // A comment between the parts of a compound is nothing: `.a/**/.b` is `.a.b`. Two names with one
            // between them are still two names in a row.
            while self.peek() == Some('\0') {
                self.bump();
            }
            if pseudo.is_some()
                && self
                    .peek()
                    .is_some_and(|c| !c.is_ascii_whitespace() && c != '>')
            {
                return Err(no(
                    "selector_invalid",
                    "a pseudo-element must end the selector",
                ));
            }
            let simple = match self.peek() {
                Some('*') => {
                    self.bump();
                    if !first {
                        return Err(no("selector_invalid", "* inside a compound"));
                    }
                    first = false;
                    continue;
                }
                Some('.') => {
                    self.bump();
                    Simple::Class(self.ident()?)
                }
                Some('#') => {
                    self.bump();
                    Simple::Id(self.ident()?)
                }
                Some('[') => {
                    self.bump();
                    self.attr()?
                }
                Some(':') => {
                    self.bump();
                    match self.pseudo()? {
                        PseudoRead::Simple(s) => s,
                        PseudoRead::Element(p) => {
                            pseudo = Some(p);
                            first = false;
                            continue;
                        }
                    }
                }
                Some(ch) if first && (name_char(ch) || ch == '\\') => {
                    Simple::Tag(self.ident()?.to_ascii_lowercase())
                }
                _ => break,
            };
            first = false;
            if c.simples.len() >= MAX_SIMPLES {
                return Err(no("selector_too_complex", "too many parts in one compound"));
            }
            c.simples.push(simple);
        }
        if c.simples.is_empty() && first {
            return Err(no("selector_invalid", "a selector has an empty part"));
        }
        Ok((c, pseudo))
    }
}

enum PseudoRead {
    Simple(Simple),
    Element(Pseudo),
}

/// Read one selector (a piece of a list). `work` counts the characters looked at.
pub fn parse(text: &str, work: &mut usize) -> Result<Selector, Refusal> {
    let mut p = P {
        s: text.trim_matches(|c: char| c.is_ascii_whitespace()),
        i: 0,
        work,
        bad: false,
    };
    let (mut compounds, mut combinators) = (Vec::new(), Vec::new());
    let pseudo = loop {
        let (c, pseudo) = p.compound()?;
        compounds.push(c);
        let spaced = p.space();
        match p.peek() {
            None => break pseudo,
            Some('>') => {
                p.bump();
                p.space();
                combinators.push(Combinator::Child);
            }
            Some('+' | '~') => {
                return Err(no(
                    "selector_sibling",
                    "sibling combinators have no static meaning here",
                ));
            }
            Some(_) if spaced => combinators.push(Combinator::Descendant),
            Some(c) => return Err(no("selector_invalid", format!("unexpected {c:?}"))),
        }
        if pseudo.is_some() {
            return Err(no(
                "selector_invalid",
                "a pseudo-element must end the selector",
            ));
        }
        if compounds.len() >= MAX_COMPOUNDS {
            return Err(no("selector_too_complex", "too many compounds"));
        }
    };
    if p.bad {
        return Err(no("selector_invalid", "a backslash that is not an escape"));
    }
    let specificity = specificity(&compounds, pseudo);
    Ok(Selector {
        compounds,
        combinators,
        pseudo,
        specificity,
    })
}

fn specificity(compounds: &[Compound], pseudo: Option<Pseudo>) -> u32 {
    let (mut a, mut b, mut c) = (0u32, 0u32, u32::from(pseudo.is_some()));
    for s in compounds.iter().flat_map(|c| &c.simples) {
        match s {
            Simple::Id(_) => a += 1,
            Simple::Class(_) | Simple::Attr(..) | Simple::Root => b += 1,
            Simple::Tag(_) => c += 1,
        }
    }
    (a.min(255) << 16) | (b.min(255) << 8) | c.min(255)
}
