//! The names an import gives: the variable name of a custom property, and the id of a variable.

use crate::scan::Section;
use std::collections::HashSet;

/// The namespaces of Tailwind v4 (`--color-*`, `--radius-*`): the first hyphen of such a name becomes a slash, so that
/// `--color-brand-500` is the variable `color/brand-500`. Longest first.
const NAMESPACES: [&str; 18] = [
    "inset-shadow",
    "drop-shadow",
    "breakpoint",
    "perspective",
    "container",
    "animate",
    "spacing",
    "tracking",
    "leading",
    "radius",
    "shadow",
    "aspect",
    "color",
    "blur",
    "font",
    "text",
    "ease",
    "z",
];

/// The variable name of a custom property: lowercase, every run of other characters one hyphen, and for `@theme` the
/// Tailwind namespace split off with a slash. `None` when nothing is left.
pub(crate) fn variable_name(raw: &str, section: Section) -> Option<String> {
    let mut base = String::new();
    for c in raw.trim_start_matches('-').chars() {
        if c.is_ascii_alphanumeric() {
            base.push(c.to_ascii_lowercase());
        } else if !base.is_empty() && !base.ends_with('-') {
            base.push('-');
        }
    }
    while base.ends_with('-') {
        base.pop();
    }
    if base.is_empty() {
        return None;
    }
    if section == Section::Theme {
        for ns in NAMESPACES {
            if let Some(rest) = base.strip_prefix(ns).and_then(|r| r.strip_prefix('-')) {
                return Some(format!("{ns}/{rest}"));
            }
        }
    }
    Some(base)
}

/// FNV-1a, 32 bits, as eight hex digits: shortens an id that is too long and keeps it the same for the same name.
fn hash8(s: &str) -> String {
    let h = s.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    format!("{h:08x}")
}

pub(crate) fn variable_id(
    collection: &str,
    name: &str,
    taken: &mut HashSet<String>,
    max: usize,
) -> String {
    let mut base = format!("var-{collection}-{}", name.replace('/', "-"));
    if base.len() > max {
        let keep = max.saturating_sub(9);
        base = format!("{}-{}", &base[..keep], hash8(&base));
    }
    let mut id = base.clone();
    let mut n = 2;
    while !taken.insert(id.clone()) {
        let tail = format!("-{n}");
        id = format!("{}{tail}", &base[..base.len().min(max - tail.len())]);
        n += 1;
    }
    id
}
