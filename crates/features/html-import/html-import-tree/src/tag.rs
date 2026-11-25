//! One tag, start or end, read from its name to its `>`: the name folded to lower case and cut, and the attributes
//! that stay (a repeated name, an event attribute and what a bound cuts are left out and counted). The first
//! occurrence of a name decides: a later one is a repeat even when the first was cut for its length. A tag that
//! reaches the end of the text before its `>` is not a tag.

use super::report::{EntryKind, Report};
use super::text::decode;
use super::{Attr, Limits, MAX_NAME_BYTES};

pub(super) struct Tag {
    pub name: String,
    pub attrs: Vec<Attr>,
    /// The byte after the `>`.
    pub end: usize,
}

/// The attributes of the tag so far: those kept, the names cut for their length, and whether the cap was reported.
#[derive(Default)]
struct Kept {
    attrs: Vec<Attr>,
    cut: Vec<String>,
    dropped: bool,
}

fn space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)
}

/// A name as the tree keeps it: ASCII lower case, NUL as the replacement character, at most [`MAX_NAME_BYTES`].
pub(super) fn fold(raw: &str) -> String {
    let mut s = String::new();
    for c in raw.chars() {
        let c = if c == '\0' {
            '\u{fffd}'
        } else {
            c.to_ascii_lowercase()
        };
        if s.len() + c.len_utf8() > MAX_NAME_BYTES {
            break;
        }
        s.push(c);
    }
    s
}

/// Read the tag whose name starts at `from`; `at` is the byte of its `<`. With `keep` false (an end tag) the
/// attributes are read past and dropped.
pub(super) fn read(
    html: &str,
    from: usize,
    at: usize,
    keep: bool,
    l: &Limits,
    report: &mut Report,
) -> Option<Tag> {
    let b = html.as_bytes();
    let n = b.len();
    let mut j = from;
    while j < n && !space(b[j]) && b[j] != b'/' && b[j] != b'>' {
        j += 1;
    }
    let name = fold(&html[from..j]);
    let mut kept = Kept::default();
    loop {
        while j < n && (space(b[j]) || b[j] == b'/') {
            j += 1;
        }
        report.work += 1;
        match b.get(j) {
            None => return None,
            Some(b'>') => {
                return Some(Tag {
                    name,
                    attrs: kept.attrs,
                    end: j + 1,
                });
            }
            Some(_) => {}
        }
        // An attribute name: one character, even `=`, then up to a space, `/`, `>` or `=`.
        let s = j;
        j += 1;
        while j < n && !space(b[j]) && !matches!(b[j], b'/' | b'>' | b'=') {
            j += 1;
        }
        let aname = &html[s..j];
        while j < n && space(b[j]) {
            j += 1;
        }
        let mut value = "";
        if b.get(j) == Some(&b'=') {
            j += 1;
            while j < n && space(b[j]) {
                j += 1;
            }
            let vs = match b.get(j) {
                None => return None,
                Some(q @ (b'"' | b'\'')) => {
                    let close = html[j + 1..].find(*q as char)?;
                    j += 1;
                    let v = j;
                    j += close + 1;
                    (v, v + close)
                }
                Some(_) => {
                    let v = j;
                    while j < n && !space(b[j]) && b[j] != b'>' {
                        j += 1;
                    }
                    (v, j)
                }
            };
            value = &html[vs.0..vs.1];
        }
        if keep {
            keep_attr(&mut kept, aname, value, at, l, report);
        }
    }
}

fn keep_attr(
    kept: &mut Kept,
    raw_name: &str,
    raw_value: &str,
    at: usize,
    l: &Limits,
    report: &mut Report,
) {
    let name = fold(raw_name);
    let max = if name == "class" || name == "style" {
        l.max_class_bytes
    } else {
        l.max_value_bytes
    };
    report.work += kept.attrs.len() + kept.cut.len() + 1;
    if kept.attrs.iter().any(|a| a.name == name) || kept.cut.contains(&name) {
        report.count("duplicate_attribute", 1);
    } else if name.len() > 2 && name.starts_with("on") {
        report.count("event_attribute", 1);
    } else if raw_value.len() > max && kept.attrs.len() + kept.cut.len() < l.max_attributes {
        // Its name is remembered, so that a later one of the name stays a repeat. Names looked at, kept or cut,
        // are at most `max_attributes`, which bounds the list.
        let msg = format!(
            "{} bytes as written; an attribute value is at most {}",
            raw_value.len(),
            max
        );
        report.note(EntryKind::Cut, "attribute_too_long", &name, msg, at);
        kept.cut.push(name);
    } else if kept.attrs.len() + kept.cut.len() >= l.max_attributes {
        if kept.dropped {
            report.count("attribute_dropped", 1);
        } else {
            kept.dropped = true;
            let msg = format!(
                "an element keeps at most {} attributes; the rest are left out",
                l.max_attributes
            );
            report.note(EntryKind::Cut, "attribute_dropped", &name, msg, at);
        }
    } else {
        let value = decode(raw_value, true, &mut report.work);
        kept.attrs.push(Attr { name, value });
    }
}
