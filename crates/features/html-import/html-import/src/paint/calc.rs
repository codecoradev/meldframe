//! The one `calc()` the style stage does not read: `calc(min(A, B, C) + N)`, which the export writes on the `::after`
//! box of a border whose radius is bound (`calc(min(var(--r) * 1px, Wpx, Hpx) + Npx)`, section 3.7). Each term is a
//! length, or a number times a length (`6 * 1px`, which is what `var(--r) * 1px` reads once the variable is
//! replaced); `N` is a length added or taken. Nothing else is evaluated.

use super::lex::split_commas;
use crate::css::value::length;
use meldframe_css::number::number;

/// The most terms of one `min()` that are read.
const MAX_TERMS: usize = 8;

/// A term of the `min()`: `12px`, `0.5rem`, `6 * 1px` or `1px * 6`, in pixels.
fn term(t: &str, em: f64, rem: f64) -> Option<f64> {
    let t = t.trim();
    let Some((a, b)) = t.split_once('*') else {
        return length(t, em, rem).map(|l| l.px);
    };
    let (a, b) = (a.trim(), b.trim());
    match (number(a), number(b)) {
        (Some(n), None) => length(b, em, rem).map(|l| l.px * n),
        (None, Some(n)) => length(a, em, rem).map(|l| l.px * n),
        _ => None,
    }
    .filter(|v| v.is_finite())
}

/// `calc(min(A, B, ...) + N)`, `- N` or no `N`, in pixels; `None` for any other text or a negative result.
pub(super) fn min_plus(text: &str, em: f64, rem: f64) -> Option<f64> {
    let lower = text.trim().to_ascii_lowercase();
    let body = lower.strip_prefix("calc(")?.strip_suffix(')')?.trim();
    let after_min = body.strip_prefix("min(")?;
    let (mut depth, mut end) = (1usize, None);
    for (i, c) in after_min.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let (args, rest) = (&after_min[..end], after_min[end + 1..].trim());
    let (parts, cut) = split_commas(args, MAX_TERMS);
    if cut || parts.is_empty() {
        return None;
    }
    let mut least = f64::INFINITY;
    for p in parts {
        least = least.min(term(p, em, rem)?);
    }
    let added = if rest.is_empty() {
        0.0
    } else {
        let (sign, n) = match (rest.strip_prefix('+'), rest.strip_prefix('-')) {
            (Some(n), _) => (1.0, n),
            (_, Some(n)) => (-1.0, n),
            _ => return None,
        };
        sign * length(n.trim(), em, rem)?.px
    };
    let v = least + added;
    (v.is_finite() && v >= 0.0).then_some(v)
}
