//! Helpers of the CSS reader tests: a selector written as one string to compare.
#![allow(dead_code)]

use html_import::css::{Combinator, Pseudo, Selector, Simple};

/// A selector as text: `div>.a #b[x=y]::after`. Specificity is written by [`spec`].
pub fn sel(s: &Selector) -> String {
    let mut out = String::new();
    for (i, c) in s.compounds.iter().enumerate() {
        if i > 0 {
            out.push(match s.combinators[i - 1] {
                Combinator::Descendant => ' ',
                Combinator::Child => '>',
            });
        }
        if c.simples.is_empty() {
            out.push('*');
        }
        for simple in &c.simples {
            match simple {
                Simple::Tag(t) => out.push_str(t),
                Simple::Class(c) => out.push_str(&format!(".{c}")),
                Simple::Id(i) => out.push_str(&format!("#{i}")),
                Simple::Attr(n, None) => out.push_str(&format!("[{n}]")),
                Simple::Attr(n, Some(v)) => out.push_str(&format!("[{n}={v}]")),
                Simple::Root => out.push_str(":root"),
            }
        }
    }
    match s.pseudo {
        Some(Pseudo::Before) => out.push_str("::before"),
        Some(Pseudo::After) => out.push_str("::after"),
        None => {}
    }
    out
}

/// The specificity as `(ids, classes, types)`.
pub fn spec(s: &Selector) -> (u32, u32, u32) {
    (
        s.specificity >> 16,
        (s.specificity >> 8) & 255,
        s.specificity & 255,
    )
}
