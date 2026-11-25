//! Values as the later stages read them: a length in pixels, a colour as hex, a value cut into its words. The
//! number, `px` and colour readers are `meldframe-css`'s (the ones `variables-import` uses); this adds `rem` (against
//! the size of the root, not always 16), `em`, `pt` and the one `calc()` the export writes, and is stricter about a bare number (only `0` is a length, as in
//! CSS). Nothing is evaluated beyond that.

use meldframe_css::color::{self, Hex};
use meldframe_css::length::length as css_length;

/// The unit a length was written in. A conversion (anything but `Px`) is for the mapping to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Px,
    Rem,
    Em,
    Pt,
    /// `calc(N * 1px)`, which is what a bound radius becomes once its variable is read.
    Calc,
}

/// A length in pixels and how it was written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Len {
    pub px: f64,
    pub unit: Unit,
}

/// A number as CSS writes it without an exponent: digits, a point and a sign. Finite.
fn number(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty()
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+'))
    {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `calc(<number> * <length in px>)` or the other way round, in any spacing: the pixels. `calc(var(--x) * 1px)` is
/// what the export writes for a bound radius, which reads `calc(8 * 1px)` once the variable is replaced; a product of
/// two numbers or of two lengths is not a length here.
fn calc_px(s: &str) -> Option<f64> {
    let body = s.strip_prefix("calc(")?.strip_suffix(')')?;
    let (a, b) = body.split_once('*')?;
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    match (a.strip_suffix("px"), b.strip_suffix("px")) {
        (Some(px), None) => Some(number(px)? * number(&b)?),
        (None, Some(px)) => Some(number(&a)? * number(px)?),
        _ => None,
    }
    .filter(|v| v.is_finite())
}

/// `12px`, `1.5rem` (`rem_px` pixels each), `2em` (`em_px` pixels each), `9pt`, `0` or the calc form above, as
/// pixels. `None` for another unit, a percentage, a keyword, a bare number other than 0, or any other function.
pub fn length(s: &str, em_px: f64, rem_px: f64) -> Option<Len> {
    let s = s.trim();
    let l = s.to_ascii_lowercase();
    if !l.starts_with("calc(") && l.contains(|c: char| c.is_ascii_whitespace()) {
        return None;
    }
    let len = |px, unit| Some(Len { px, unit });
    if l.starts_with("calc(") {
        return calc_px(&l).and_then(|px| len(px, Unit::Calc));
    }
    if let Some(n) = l.strip_suffix("pt") {
        return number(n).and_then(|v| len(v * 4.0 / 3.0, Unit::Pt));
    }
    if let Some(n) = l.strip_suffix("rem") {
        return number(n).and_then(|v| len(v * rem_px, Unit::Rem));
    }
    if let Some(n) = l.strip_suffix("em") {
        return number(n).and_then(|v| len(v * em_px, Unit::Em));
    }
    let v = css_length(s)?;
    (l.ends_with("px") || v.value == 0.0).then_some(Len {
        px: v.value,
        unit: Unit::Px,
    })
}

/// A colour as the model holds it: hex in lower case (kept as written otherwise), a keyword or a function that
/// `meldframe-css` reads. `None` for `currentcolor` (the caller knows what it stands for), a system colour, `lab()`
/// and the rest.
pub fn colour(s: &str) -> Option<Hex> {
    let s = s.trim();
    if color::is_hex(s) {
        return Some(Hex {
            hex: s.to_ascii_lowercase(),
            clipped: false,
        });
    }
    color::named(s).or_else(|| color::function(s))
}

/// A value cut at the white space outside parentheses and strings: `1px solid rgb(0 0 0)` is three words. Words
/// are slices of `s`.
pub fn words(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start, mut quote) =
        (Vec::new(), 0usize, None::<usize>, None::<u8>);
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 1;
            } else if c == q {
                quote = None;
            }
        } else if c.is_ascii_whitespace() && depth == 0 {
            if let Some(st) = start.take() {
                out.push(&s[st..i]);
            }
            i += 1;
            continue;
        } else {
            match c {
                b'"' | b'\'' => quote = Some(c),
                b'\\' => i += 1,
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        start.get_or_insert(i);
        i += 1;
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}

/// Is there a `,` outside parentheses and strings: a list of layers or of values.
pub fn has_top_level_comma(s: &str) -> bool {
    let (mut depth, mut quote, b) = (0usize, None::<u8>, s.as_bytes());
    let mut i = 0;
    while i < b.len() {
        match (quote, b[i]) {
            (_, b'\\') => i += 1,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'(') => depth += 1,
            (None, b')') => depth = depth.saturating_sub(1),
            (None, b',') if depth == 0 => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

/// `inherit`, `initial`, `unset` or `revert`, in any case.
pub fn is_wide(s: &str) -> bool {
    ["inherit", "initial", "unset", "revert"]
        .iter()
        .any(|k| s.trim().eq_ignore_ascii_case(k))
}
