//! The builder: one pass over the text, a stack of open elements, and the rules of `rules.rs`. Every step moves
//! forward in the text, or ends the read, and every look back is at most `max_depth` open elements, so the work is
//! linear in the text.

use super::report::{EntryKind, Report};
use super::rules::{self, Raw};
use super::tag::{self, Tag};
use super::text::decode;
use super::{Child, Element, ElementId, Limits, ParseError, Parsed, Tree};

/// An open element. `id` is `None` for one the depth limit did not make: it is open (its end tag closes it) but its
/// content goes into the nearest element that was made.
struct Frame {
    tag: String,
    id: Option<ElementId>,
}

struct Builder<'a> {
    html: &'a str,
    l: &'a Limits,
    elements: Vec<Element>,
    stack: Vec<Frame>,
    /// The nearest element that was made; the content goes here.
    cur: ElementId,
    /// The elements on the stack that were made.
    depth: usize,
    /// The text at the end of `cur` was cut by the limit and has been counted.
    cut: bool,
    report: Report,
}

pub(super) fn build(html: &str, l: &Limits) -> Result<Parsed, ParseError> {
    let root = Element {
        tag: String::new(),
        attrs: Vec::new(),
        children: Vec::new(),
        parent: ElementId(0),
        at: 0,
    };
    let mut b = Builder {
        html,
        l,
        elements: vec![root],
        stack: Vec::new(),
        cur: ElementId(0),
        depth: 0,
        cut: false,
        report: Report::default(),
    };
    b.run()?;
    if b.elements.len() == 1 {
        return Err(ParseError::Empty);
    }
    Ok(Parsed {
        tree: Tree {
            elements: b.elements,
        },
        report: b.report,
    })
}

impl Builder<'_> {
    fn run(&mut self) -> Result<(), ParseError> {
        let html = self.html;
        let n = html.len();
        let b = html.as_bytes();
        let mut i = 0;
        while i < n {
            self.report.work += 1;
            let next = html[i..].find('<').map_or(n, |k| i + k);
            if next > i {
                self.text(&html[i..next], true, i);
                i = next;
                continue;
            }
            let c = b.get(i + 1).copied().unwrap_or(0);
            i = match c {
                b'!' if html[i..].starts_with("<!--") => self.comment(i),
                b'!' | b'?' => self.bogus(i, 2),
                b'/' if b.get(i + 2).is_some_and(u8::is_ascii_alphabetic) => self.end_tag(i),
                b'/' => self.bogus(i, 2),
                c if c.is_ascii_alphabetic() => self.start_tag(i)?,
                _ => {
                    self.text("<", false, i);
                    i + 1
                }
            };
        }
        // What is still open at the end is closed there.
        let open = self.stack.len();
        if open > 0 {
            let msg = format!(
                "{open} elements were still open at the end of the text and are closed there"
            );
            self.report.count("unclosed_at_end", open - 1);
            self.report
                .note(EntryKind::Recovered, "unclosed_at_end", "", msg, n);
            self.close_to(0);
        }
        Ok(())
    }

    /// A comment from the `<!--` at `i`; the byte after it.
    fn comment(&mut self, i: usize) -> usize {
        // The `--` of the opener may belong to the closer: `<!-->` and `<!--->` are empty comments.
        match self.html[i + 2..].find("-->") {
            Some(k) => {
                self.report.count("comment", 1);
                i + 2 + k + 3
            }
            None => self.unterminated("unterminated_comment", i, "a comment"),
        }
    }

    /// A doctype, a processing instruction, CDATA or `</` without a name: dropped up to the next `>`.
    fn bogus(&mut self, i: usize, skip: usize) -> usize {
        let rest = &self.html[i + skip..];
        let doctype = rest.len() >= 7 && rest.as_bytes()[..7].eq_ignore_ascii_case(b"doctype");
        match rest.find('>') {
            Some(k) => {
                self.report
                    .count(if doctype { "doctype" } else { "comment" }, 1);
                i + skip + k + 1
            }
            None => self.unterminated("unterminated_comment", i, "a declaration"),
        }
    }

    /// Something that does not end before the text does: the read ends there.
    fn unterminated(&mut self, code: &'static str, at: usize, what: &str) -> usize {
        let msg = format!(
            "{what} that starts at byte {at} is not finished before the end of the text; the rest is dropped"
        );
        self.report.note(EntryKind::Recovered, code, "", msg, at);
        self.html.len()
    }

    fn end_tag(&mut self, i: usize) -> usize {
        let Some(Tag { name, end, .. }) =
            tag::read(self.html, i + 2, i, false, self.l, &mut self.report)
        else {
            return self.unterminated("unterminated_tag", i, "an end tag");
        };
        let n = self.stack.len();
        let lo = n.saturating_sub(self.l.max_depth);
        let found = (lo..n).rev().find(|&k| {
            self.report.work += 1;
            self.stack[k].tag == name
        });
        match found {
            None => {
                let msg = "an end tag with no open element of its name is dropped".to_string();
                self.report
                    .note(EntryKind::Recovered, "stray_end_tag", &name, msg, i);
            }
            Some(k) => {
                if k + 1 < n {
                    let msg = format!(
                        "the end tag closes {} element(s) that were still open inside it",
                        n - k - 1
                    );
                    self.report
                        .note(EntryKind::Recovered, "misnested_end_tag", &name, msg, i);
                }
                self.close_to(k);
            }
        }
        end
    }

    fn start_tag(&mut self, i: usize) -> Result<usize, ParseError> {
        let Some(Tag { name, attrs, end }) =
            tag::read(self.html, i + 1, i, true, self.l, &mut self.report)
        else {
            return Ok(self.unterminated("unterminated_tag", i, "a tag"));
        };
        self.close_implied(&name);
        if let Some(raw) = rules::raw(&name) {
            return self.raw_element(raw, name, attrs, i, end);
        }
        let void = rules::is_void(&name);
        self.open(name, attrs, i, void)?;
        Ok(end)
    }

    /// An element whose content is read as raw text, up to its own end tag: the byte after the end tag.
    fn raw_element(
        &mut self,
        raw: Raw,
        name: String,
        attrs: Vec<super::Attr>,
        at: usize,
        from: usize,
    ) -> Result<usize, ParseError> {
        let (content_end, end) = match find_end(self.html, from, &name) {
            Some(found) => found,
            None => {
                let msg = format!(
                    "<{}> is not closed before the end of the text; the rest is its content",
                    name
                );
                self.report.note(
                    EntryKind::Recovered,
                    "unterminated_raw_text",
                    &name,
                    msg,
                    at,
                );
                (self.html.len(), self.html.len())
            }
        };
        if raw == Raw::Script {
            self.report.count("script", 1);
            return Ok(end);
        }
        self.open(name, attrs, at, false)?;
        self.text(&self.html[from..content_end], raw == Raw::Text, from);
        self.close_to(self.stack.len() - 1);
        Ok(end)
    }

    /// Make an element in `cur` (and open it unless it is void), or flatten it when it is too deep.
    fn open(
        &mut self,
        name: String,
        attrs: Vec<super::Attr>,
        at: usize,
        void: bool,
    ) -> Result<(), ParseError> {
        if self.depth >= self.l.max_depth {
            let msg = format!(
                "the element is deeper than {}; its content goes into the element it is in",
                self.l.max_depth
            );
            self.report
                .note(EntryKind::Recovered, "depth_cut", &name, msg, at);
            if !void {
                self.stack.push(Frame {
                    tag: name,
                    id: None,
                });
            }
            return Ok(());
        }
        if self.elements.len() > self.l.max_elements {
            return Err(ParseError::TooMany {
                max: self.l.max_elements,
                at,
            });
        }
        let id = ElementId(self.elements.len());
        self.elements.push(Element {
            tag: name.clone(),
            attrs,
            children: Vec::new(),
            parent: self.cur,
            at,
        });
        self.elements[self.cur.0].children.push(Child::Element(id));
        if !void {
            self.stack.push(Frame {
                tag: name,
                id: Some(id),
            });
            self.cur = id;
            self.depth += 1;
        }
        Ok(())
    }

    /// The open elements a start tag of `name` ends, by the rules.
    fn close_implied(&mut self, name: &str) {
        let Some(rule) = rules::implied(name) else {
            return;
        };
        let n = self.stack.len();
        let lo = n.saturating_sub(if rule.top_only { 1 } else { self.l.max_depth });
        for k in (lo..n).rev() {
            self.report.work += 1;
            let tag = self.stack[k].tag.as_str();
            if rule.closes.contains(&tag) {
                self.close_to(k);
                return;
            }
            if rule.stops.contains(&tag) {
                return;
            }
        }
    }

    /// Close the open elements from the one at `k` up.
    fn close_to(&mut self, k: usize) {
        while self.stack.len() > k {
            self.report.work += 1;
            if let Some(f) = self.stack.pop()
                && let Some(id) = f.id
            {
                self.cur = self.elements[id.0].parent;
                self.depth -= 1;
            }
        }
    }

    /// Add `raw`, from the byte `at` of the text, to the text at the end of `cur`, or start one. Past the limit it is
    /// cut.
    fn text(&mut self, raw: &str, references: bool, at: usize) {
        let mut s = decode(raw, references, &mut self.report.work);
        let cur = self.cur;
        let kids = &mut self.elements[cur.0].children;
        let last = match kids.last_mut() {
            Some(Child::Text(t)) => Some(t),
            _ => None,
        };
        // A style sheet is bounded by the input; any other text by `max_text_bytes`.
        let in_style = self
            .stack
            .last()
            .is_some_and(|f| f.tag == "style" && f.id == Some(cur));
        let room = if in_style {
            usize::MAX
        } else {
            self.l
                .max_text_bytes
                .saturating_sub(last.as_ref().map_or(0, |t| t.len()))
        };
        let mut cut = false;
        if s.len() > room {
            let mut k = room;
            while !s.is_char_boundary(k) {
                k -= 1;
            }
            s.truncate(k);
            cut = true;
        }
        match last {
            Some(t) => t.push_str(&s),
            None => {
                self.cut = false;
                if !s.is_empty() {
                    kids.push(Child::Text(s));
                }
            }
        }
        if cut && !self.cut {
            self.cut = true;
            let msg = format!("a text is cut at {} bytes", self.l.max_text_bytes);
            self.report.note(EntryKind::Cut, "text_cut", "", msg, at);
        }
    }
}

/// Where the raw text that starts at `from` ends: the byte of its end tag `</name`, in any case and followed by a
/// space, `/` or `>`, and the byte after that tag's `>`. `None` when there is none.
fn find_end(html: &str, from: usize, name: &str) -> Option<(usize, usize)> {
    let b = html.as_bytes();
    let mut p = from;
    while let Some(k) = html[p..].find("</") {
        let s = p + k;
        let nm = s + 2 + name.len();
        if b.len() > nm
            && b[s + 2..nm].eq_ignore_ascii_case(name.as_bytes())
            && matches!(b[nm], b' ' | b'\t' | b'\n' | b'\r' | 0x0c | b'/' | b'>')
        {
            return html[nm..].find('>').map(|g| (s, nm + g + 1));
        }
        p = s + 2;
    }
    None
}
