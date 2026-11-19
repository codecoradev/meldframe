//! The names and the numbers an export writes.

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
