//! Text as the tree keeps it: line ends made line feeds, NUL made the replacement character, and the character
//! references of a small subset read. A reference is looked for in a window of a few bytes after its `&` and never
//! read again after it is replaced, and what it stands for is never longer than it is written, so the result is
//! never longer than the text and nothing grows by repeating a reference. The one exception is NUL: a byte becomes
//! the three bytes of U+FFFD, so what is stored is at most three times the text. A value limit counts the bytes
//! as written, a text limit the bytes as stored (a NUL is three, a reference is what it stands for).

/// The most digits of a numeric reference; one with more is left as written.
const MAX_DIGITS: usize = 8;

/// The most bytes of a name; the table below has none longer.
const MAX_NAMED: usize = 8;

const NAMED: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{a0}'),
    ("copy", '\u{a9}'),
    ("reg", '\u{ae}'),
    ("trade", '\u{2122}'),
    ("hellip", '\u{2026}'),
    ("mdash", '\u{2014}'),
    ("ndash", '\u{2013}'),
    ("laquo", '\u{ab}'),
    ("raquo", '\u{bb}'),
    ("lsquo", '\u{2018}'),
    ("rsquo", '\u{2019}'),
    ("ldquo", '\u{201c}'),
    ("rdquo", '\u{201d}'),
    ("bull", '\u{2022}'),
    ("middot", '\u{b7}'),
    ("times", '\u{d7}'),
    ("divide", '\u{f7}'),
    ("deg", '\u{b0}'),
    ("plusmn", '\u{b1}'),
    ("euro", '\u{20ac}'),
    ("pound", '\u{a3}'),
    ("yen", '\u{a5}'),
    ("cent", '\u{a2}'),
    ("sect", '\u{a7}'),
    ("para", '\u{b6}'),
    ("larr", '\u{2190}'),
    ("rarr", '\u{2192}'),
    ("uarr", '\u{2191}'),
    ("darr", '\u{2193}'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
    ("thinsp", '\u{2009}'),
    ("shy", '\u{ad}'),
];

/// `raw` as the tree keeps it. With `references` false (the content of a `<style>`) only the line ends and NUL
/// change. `work` is the count of bytes looked at after an `&`.
pub(super) fn decode(raw: &str, references: bool, work: &mut usize) -> String {
    let b = raw.as_bytes();
    if !b.iter().any(|c| matches!(c, b'\r' | 0 | b'&')) {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let (mut run, mut i) = (0, 0);
    while i < b.len() {
        match b[i] {
            b'\r' => {
                out.push_str(&raw[run..i]);
                out.push('\n');
                i += if b.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
                run = i;
            }
            0 => {
                out.push_str(&raw[run..i]);
                out.push('\u{fffd}');
                i += 1;
                run = i;
            }
            b'&' if references => match reference(&raw[i..], work) {
                Some((c, len)) => {
                    out.push_str(&raw[run..i]);
                    out.push(c);
                    i += len;
                    run = i;
                }
                None => i += 1,
            },
            _ => i += 1,
        }
    }
    out.push_str(&raw[run..]);
    out
}

/// The character `s` (which starts with `&`) stands for and how many bytes the reference takes, or `None` when it
/// is not one of the subset: a name that is not in the table, a number with too many digits, no `;`.
fn reference(s: &str, work: &mut usize) -> Option<(char, usize)> {
    let b = s.as_bytes();
    if b.get(1) == Some(&b'#') {
        let hex = matches!(b.get(2), Some(b'x' | b'X'));
        let start = if hex { 3 } else { 2 };
        let radix = if hex { 16 } else { 10 };
        let mut end = start;
        let mut value = 0u32;
        while let Some(d) = b.get(end).and_then(|c| (*c as char).to_digit(radix)) {
            *work += 1;
            if end - start >= MAX_DIGITS {
                return None;
            }
            value = value * radix + d;
            end += 1;
        }
        if end == start || b.get(end) != Some(&b';') {
            return None;
        }
        // This is how a browser reads a reference to a control character (it is the replacement character), so it is
        // part of reading the document and not a cleaning of text for display (rule U9): the text the tree keeps is
        // file content and keeps every other character, hidden ones included, as the user wrote it. The display layer
        // makes it safe where it shows it.
        let c = char::from_u32(value)
            .filter(|c| !c.is_control() || matches!(c, '\t' | '\n'))
            .unwrap_or('\u{fffd}');
        return Some((c, end + 1));
    }
    let mut end = 1;
    while end < b.len() && end <= MAX_NAMED && b[end].is_ascii_alphanumeric() {
        *work += 1;
        end += 1;
    }
    *work += 1;
    if end == 1 || b.get(end) != Some(&b';') {
        return None;
    }
    let name = &s[1..end];
    NAMED
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| (*c, end + 1))
}
