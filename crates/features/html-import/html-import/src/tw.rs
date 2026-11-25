//! The Tailwind class decoder: the inverse of `crates/features/export/export/src/tw.rs` and of nothing else
//! (`docs/import-html.md` sections 2.2 and 3.3). The export writes a fixed vocabulary, and this reads exactly it
//! back into the declarations the CSS reader produces, so that one cascade serves both:
//!
//! - the named utilities (`flex`, `flex-row`, `flex-col`, `shrink-0`, `overflow-hidden`, `self-stretch`,
//!   `items-start|center|end`, `justify-start|between`, `text-center|right`, `whitespace-pre|pre-wrap`, `w-fit`,
//!   `w-auto`, `h-auto`, `absolute`, `relative`, `pointer-events-none`, `content-['']`);
//! - the arbitrary-value utilities (`left-[..]`, `top-[..]`, `inset-[..]`, `w-[..]`, `h-[..]`, `min-w-[..]`,
//!   `min-h-[..]`, `gap-[..]`, `p-[..]`, `m-[..]`, `flex-[..]`, `text-[length:..]`, `font-[..]`, `leading-[..]`,
//!   `text-[color:..]`, `bg-[color:..]`, `rounded-[..]`, `opacity-[..]`, `shadow-[..]`);
//! - the arbitrary property `[prop:value]` (also a custom property);
//! - the `after:` variant, which is the `::after` box.
//!
//! In a value `_` is a space and `\_` an underscore. Everything else is unresolved, never guessed: the scale and
//! the palette (`p-4`, `text-lg`, `bg-blue-500`, `rounded-lg`), any other variant (`hover:`, `md:`, `dark:`,
//! `before:`), an important or negative form, and a value that cannot be a CSS value here. The agent that has the
//! project's compiled CSS sends it as `css`, where those classes are ordinary rules.

use crate::css::Pseudo;

/// A class read: the box it styles and its declarations (`name`, `value`), in the order of the utility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub pseudo: Option<Pseudo>,
    pub decls: Vec<(String, String)>,
}

/// Why a class is not read: a stable reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unresolved(pub &'static str);

/// The utilities with no value, as `(class, property, value)`.
const FIXED: &[(&str, &str, &str)] = &[
    ("absolute", "position", "absolute"),
    ("relative", "position", "relative"),
    ("flex", "display", "flex"),
    ("flex-row", "flex-direction", "row"),
    ("flex-col", "flex-direction", "column"),
    ("shrink-0", "flex-shrink", "0"),
    ("overflow-hidden", "overflow", "hidden"),
    ("pointer-events-none", "pointer-events", "none"),
    ("self-stretch", "align-self", "stretch"),
    ("items-start", "align-items", "flex-start"),
    ("items-center", "align-items", "center"),
    ("items-end", "align-items", "flex-end"),
    ("justify-start", "justify-content", "flex-start"),
    ("justify-between", "justify-content", "space-between"),
    ("text-center", "text-align", "center"),
    ("text-right", "text-align", "right"),
    ("whitespace-pre", "white-space", "pre"),
    ("whitespace-pre-wrap", "white-space", "pre-wrap"),
    ("w-fit", "width", "fit-content"),
    ("w-auto", "width", "auto"),
    ("h-auto", "height", "auto"),
    ("content-['']", "content", "\"\""),
];

/// The utilities that take a value: where the value starts (the prefix includes its `[`) and the property.
const PREFIXED: &[(&str, &str)] = &[
    ("left-[", "left"),
    ("top-[", "top"),
    ("inset-[", "inset"),
    ("w-[", "width"),
    ("h-[", "height"),
    ("min-w-[", "min-width"),
    ("min-h-[", "min-height"),
    ("gap-[", "gap"),
    ("p-[", "padding"),
    ("m-[", "margin"),
    ("flex-[", "flex"),
    ("text-[length:", "font-size"),
    ("font-[", "font-weight"),
    ("leading-[", "line-height"),
    ("text-[color:", "color"),
    ("bg-[color:", "background-color"),
    ("rounded-[", "border-radius"),
    ("opacity-[", "opacity"),
    ("shadow-[", "box-shadow"),
];

/// A value of an arbitrary utility: `_` is a space, `\_` is an underscore. Refused when there is nothing, when it is
/// over the limit, or when it holds a character that ends a declaration or a rule.
fn value(v: &str, max: usize) -> Result<String, Unresolved> {
    if v.is_empty() {
        return Err(Unresolved("empty_value"));
    }
    if v.len() > max {
        return Err(Unresolved("value_too_long"));
    }
    if v.contains([';', '{', '}']) {
        return Err(Unresolved("invalid_value"));
    }
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'_') => {
                chars.next();
                out.push('_');
            }
            '_' => out.push(' '),
            c => out.push(c),
        }
    }
    Ok(out)
}

/// The index of the first `:` that is not inside `[...]`: where a variant ends.
fn variant_end(class: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in class.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

fn property(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
}

/// Read one class. `max_value` is the most bytes of a value (the limit of a CSS value).
pub fn decode(class: &str, max_value: usize) -> Result<Decoded, Unresolved> {
    let (pseudo, rest) = match variant_end(class) {
        None => (None, class),
        Some(i) if &class[..i] == "after" => (Some(Pseudo::After), &class[i + 1..]),
        Some(_) => return Err(Unresolved("variant")),
    };
    let one = |p: &str, v: String| Decoded {
        pseudo,
        decls: vec![(p.to_owned(), v)],
    };
    if let Some((_, p, v)) = FIXED.iter().find(|(c, _, _)| *c == rest) {
        return Ok(one(p, (*v).to_owned()));
    }
    if let Some((prefix, p)) = PREFIXED.iter().find(|(pre, _)| rest.starts_with(pre)) {
        let v = rest[prefix.len()..]
            .strip_suffix(']')
            .ok_or(Unresolved("not_closed"))?;
        return Ok(one(p, value(v, max_value)?));
    }
    if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']'))
        && let Some((name, v)) = inner.split_once(':')
        && property(name)
    {
        let name = if name.starts_with("--") {
            name.to_owned()
        } else {
            name.to_ascii_lowercase()
        };
        return Ok(one(&name, value(v, max_value)?));
    }
    Err(Unresolved("not_in_vocabulary"))
}
