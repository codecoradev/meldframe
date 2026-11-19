//! The names an export gives: CSS variables, and the attributes that pick a mode.

use std::collections::HashSet;

/// Lowercase ASCII letters and digits, with every other run of characters as one hyphen; `x` when nothing is left.
pub(crate) fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "x".into() } else { out }
}

/// `base`, or `base-2`, `base-3`... when `taken` already holds it; the answer is added to `taken`.
pub(crate) fn unique(taken: &mut HashSet<String>, base: String) -> String {
    let mut name = base.clone();
    let mut n = 2;
    while !taken.insert(name.clone()) {
        name = format!("{base}-{n}");
        n += 1;
    }
    name
}

/// A number as CSS writes it: the shortest form, `0` for zero and negative zero.
pub(crate) fn num(v: f64) -> String {
    if v == 0.0 { "0".into() } else { format!("{v}") }
}

pub(crate) fn px(v: f64) -> String {
    format!("{}px", num(v))
}

/// `&`, `<`, `>` and `"` as entities, for text and for attribute values.
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// The `#rrggbb` of a colour the render model painted.
pub(crate) fn hex(rgb: u32) -> String {
    format!("#{rgb:06x}")
}

/// `s` for CSS: ASCII letters, digits and the characters of `keep` as they are, every other character as a hex
/// escape (`\3c ` for `<`, with the space that ends it), so that nothing a user wrote can end a string, a rule, a
/// `url(...)` or the `<style>` element that holds the stylesheet.
pub(crate) fn css_escape(s: &str, keep: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || keep.contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("\\{:x} ", c as u32));
        }
    }
    out
}

/// The text of a hex colour (`#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`), else `None`: the only colour text the
/// export copies into a stylesheet.
pub(crate) fn hex_colour(s: &str) -> Option<&str> {
    let h = s.strip_prefix('#')?;
    (matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())).then_some(s)
}

/// An asset id as the file stores it: 64 lowercase hex digits.
pub(crate) fn asset_id_ok(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
