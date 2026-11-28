//! The small readers every paint value shares: a list cut at its commas, a number, a percentage, the inside of a
//! function. They look at the text once and allocate only the slices they return.

use meldframe_css::number::number;

/// `s` cut at the commas outside parentheses and strings, each piece trimmed, at most `max` pieces; the flag says
/// that more were left out. An empty text is no pieces.
pub(super) fn split_commas(s: &str, max: usize) -> (Vec<&str>, bool) {
    let (mut out, mut depth, mut quote, mut start) = (Vec::new(), 0usize, None::<u8>, 0usize);
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match (quote, b[i]) {
            (_, b'\\') => i += 1,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'(') => depth += 1,
            (None, b')') => depth = depth.saturating_sub(1),
            (None, b',') if depth == 0 => {
                if out.len() >= max {
                    return (out, true);
                }
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    let last = s[start.min(s.len())..].trim();
    if out.is_empty() && last.is_empty() {
        return (out, false);
    }
    if out.len() >= max {
        return (out, true);
    }
    out.push(last);
    (out, false)
}

/// `50%` as 50.
pub(super) fn percent(s: &str) -> Option<f64> {
    number(s.trim().strip_suffix('%')?)
}

/// What is between the parentheses of `name(...)` when `s` is exactly that one call (any case for the name): the
/// closing parenthesis is the last byte and the first one opened is the one it closes.
pub(super) fn inner<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let s = s.trim();
    let head = s.get(..name.len())?;
    if !head.eq_ignore_ascii_case(name) || s.as_bytes().get(name.len()) != Some(&b'(') {
        return None;
    }
    let body = s[name.len() + 1..].strip_suffix(')')?;
    let (mut depth, mut quote) = (0usize, None::<u8>);
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match (quote, b[i]) {
            (_, b'\\') => i += 1,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'(') => depth += 1,
            (None, b')') if depth == 0 => return None,
            (None, b')') => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    (depth == 0).then_some(body)
}

/// A colour word as the model holds it: `#rrggbb[aa]` (or the short hex as written), and whether a colour outside
/// sRGB was clamped. `currentcolor` is the colour of the element. `None` for anything else (a system colour, `lab()`,
/// a word that is no colour).
pub(super) fn colour_word(w: &str, style: &crate::css::Style) -> Option<(String, bool)> {
    let w = w.trim();
    if w.eq_ignore_ascii_case("currentcolor") {
        return Some((style.color.clone(), false));
    }
    crate::css::value::colour(w).map(|h| (h.hex, h.clipped))
}
