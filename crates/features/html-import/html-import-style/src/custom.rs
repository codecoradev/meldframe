//! The custom properties (`--x`) of an element: the parent's, with what the element declares resolved over them, on
//! the element that declares them (see `style.rs` for the rules).

use crate::computed::{Custom, Note, Style};
use crate::style::Cx;
use crate::value::is_wide;
use crate::vars::resolve;
use html_import_css::css::Candidate;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wide {
    Inherit,
    Initial,
    Unset,
    Revert,
}

pub(crate) fn wide(v: &str) -> Option<Wide> {
    if !is_wide(v) {
        return None;
    }
    let v = v.trim();
    Some(if v.eq_ignore_ascii_case("inherit") {
        Wide::Inherit
    } else if v.eq_ignore_ascii_case("initial") {
        Wide::Initial
    } else if v.eq_ignore_ascii_case("unset") {
        Wide::Unset
    } else {
        Wide::Revert
    })
}

pub(crate) fn has_var(v: &str) -> bool {
    v.as_bytes()
        .windows(4)
        .any(|w| w.eq_ignore_ascii_case(b"var("))
}

/// What a custom property of an element came to.
enum Entry {
    Value(Arc<str>),
    Invalid,
    Absent,
}

/// The custom properties of an element: the parent's, with what the element declares resolved over them.
pub(crate) fn customs(
    cx: &mut Cx,
    notes: &mut Vec<Note>,
    parent: &Style,
    all: &[&Candidate],
) -> Custom {
    let mut winners: BTreeMap<Arc<str>, Arc<str>> = BTreeMap::new();
    for c in all.iter().filter(|c| c.name.starts_with("--")) {
        winners.insert(c.name.clone(), c.value.clone());
    }
    if winners.is_empty() {
        return parent.custom.clone();
    }
    let mut done: BTreeMap<Arc<str>, Entry> = BTreeMap::new();
    // The CSS-wide keywords first, so that a value that names one of them finds what it came to.
    for (name, raw) in &winners {
        let entry = match wide(raw) {
            Some(Wide::Initial) => Entry::Invalid,
            Some(_) => match parent.custom.get(name) {
                Some(Some(v)) => Entry::Value(v.clone()),
                Some(None) => Entry::Invalid,
                None => Entry::Absent,
            },
            None => continue,
        };
        done.insert(name.clone(), entry);
    }
    let pending: Vec<(&Arc<str>, &Arc<str>)> = winners
        .iter()
        .filter(|(n, _)| !done.contains_key(*n))
        .collect();
    for (name, raw) in pending {
        let lookup = |n: &str| match done.get(n) {
            Some(Entry::Value(v)) => Some(v.clone()),
            Some(_) => None,
            None => match winners.get(n) {
                Some(w) => Some(w.clone()),
                None => parent.custom.get(n).cloned().flatten(),
            },
        };
        let (r, cost) = resolve(raw, &lookup, cx.l.max_value_bytes);
        cx.charge(cost);
        let entry = match r {
            Ok(v) if has_var(raw) => Entry::Value(v.into()),
            Ok(_) => Entry::Value(raw.clone()),
            Err(e) => {
                cx.note(notes, name, raw, e.code());
                Entry::Invalid
            }
        };
        done.insert(name.clone(), entry);
    }
    let mut map = (*parent.custom).clone();
    cx.steps += map.len();
    cx.bytes += map.len() * crate::style::ENTRY_BYTES;
    for (name, entry) in done {
        match entry {
            Entry::Value(v) => map.insert(name, Some(v)),
            Entry::Invalid => map.insert(name, None),
            Entry::Absent => map.remove(&name),
        };
    }
    Arc::new(map)
}
