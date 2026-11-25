//! The custom properties (`--x`) of an element: the parent's, with what the element declares resolved over them, on
//! the element that declares them (see `style.rs` for the rules).

use crate::computed::{Custom, Note};
use crate::style::Cx;
use crate::value::is_wide;
use crate::vars::{MAX_DEPTH, VarError, resolve};
use html_import_css::css::Candidate;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
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

/// A custom property that could not be resolved: its name, its value and the code of the note.
type Failure = (Arc<str>, Arc<str>, &'static str);

/// What a custom property of an element came to.
enum Entry {
    Value(Arc<str>),
    Invalid,
    Absent,
}

/// The custom properties an element declares, resolved depth first: the value of one that another names is
/// resolved when it is asked for, and one that is on the stack (a cycle) or is more than [`MAX_DEPTH`] deep is
/// guaranteed-invalid, so a `var()` that names it uses its fallback, whatever the order of the names.
struct Resolver<'a> {
    winners: &'a BTreeMap<Arc<str>, Arc<str>>,
    parent: &'a Custom,
    max_len: usize,
    done: RefCell<BTreeMap<Arc<str>, Entry>>,
    stack: RefCell<Vec<Arc<str>>>,
    steps: Cell<usize>,
    cycles: Cell<usize>,
    /// The names that are in a cycle: invalid, whatever fallback their own `var()` has.
    cyclic: RefCell<BTreeSet<Arc<str>>>,
    notes: RefCell<Vec<Failure>>,
}

impl Resolver<'_> {
    /// The literal of custom property `n` as a `var()` of this element sees it: what this element declares, resolved
    /// now if it was not yet, else what the parent has.
    fn value_of(&self, n: &str) -> Option<Arc<str>> {
        if let Some(e) = self.done.borrow().get(n) {
            return match e {
                Entry::Value(v) => Some(v.clone()),
                _ => None,
            };
        }
        let Some((name, raw)) = self.winners.get_key_value(n) else {
            return self.parent.get(n).cloned().flatten();
        };
        let at = self.stack.borrow().iter().position(|s| **s == *n);
        if let Some(at) = at {
            // Everything from there up is in the cycle, and the fallback of a `var()` in it does not save it.
            let stack = self.stack.borrow();
            self.cyclic.borrow_mut().extend(stack[at..].iter().cloned());
        }
        if at.is_some() || self.stack.borrow().len() >= MAX_DEPTH {
            self.cycles.set(self.cycles.get() + 1);
            return None;
        }
        self.resolve_winner(name, raw);
        match self.done.borrow().get(n) {
            Some(Entry::Value(v)) => Some(v.clone()),
            _ => None,
        }
    }

    fn resolve_winner(&self, name: &Arc<str>, raw: &Arc<str>) {
        self.stack.borrow_mut().push(name.clone());
        let before = self.cycles.get();
        let (r, cost) = resolve(raw, &|n| self.value_of(n), self.max_len);
        self.stack.borrow_mut().pop();
        self.steps
            .set(self.steps.get() + 1 + cost.expansions + cost.scanned / 64);
        let entry = match r {
            Ok(_) if self.cyclic.borrow().contains(name) => {
                self.notes
                    .borrow_mut()
                    .push((name.clone(), raw.clone(), "var_too_deep"));
                Entry::Invalid
            }
            Ok(v) if has_var(raw) => Entry::Value(v.into()),
            Ok(_) => Entry::Value(raw.clone()),
            Err(e) => {
                // A name that was a cycle, or too deep, is why it is not there.
                let code = match e {
                    VarError::Undefined(_) if self.cycles.get() > before => "var_too_deep",
                    e => e.code(),
                };
                self.notes
                    .borrow_mut()
                    .push((name.clone(), raw.clone(), code));
                Entry::Invalid
            }
        };
        self.done.borrow_mut().insert(name.clone(), entry);
    }
}

/// The custom properties of an element (or of one of its generated boxes): `parent` with what `all` declares
/// resolved over them. The last declaration of a name wins.
pub(crate) fn customs(
    cx: &mut Cx,
    notes: &mut Vec<Note>,
    parent: &Custom,
    all: &[&Candidate],
) -> Custom {
    let mut winners: BTreeMap<Arc<str>, Arc<str>> = BTreeMap::new();
    for c in all.iter().filter(|c| c.name.starts_with("--")) {
        winners.insert(c.name.clone(), c.value.clone());
    }
    if winners.is_empty() {
        return parent.clone();
    }
    let r = Resolver {
        winners: &winners,
        parent,
        max_len: cx.l.max_value_bytes,
        done: RefCell::new(BTreeMap::new()),
        stack: RefCell::new(Vec::new()),
        steps: Cell::new(0),
        cycles: Cell::new(0),
        cyclic: RefCell::new(BTreeSet::new()),
        notes: RefCell::new(Vec::new()),
    };
    // The CSS-wide keywords first, so that a value that names one of them finds what it came to: `initial` is
    // guaranteed-invalid (not absent: it hides the parent's), `inherit` and `unset` are the parent's.
    for (name, raw) in &winners {
        let entry = match wide(raw) {
            Some(Wide::Initial) => Entry::Invalid,
            Some(_) => match parent.get(name) {
                Some(Some(v)) => Entry::Value(v.clone()),
                Some(None) => Entry::Invalid,
                None => Entry::Absent,
            },
            None => continue,
        };
        r.done.borrow_mut().insert(name.clone(), entry);
    }
    for (name, raw) in &winners {
        if !r.done.borrow().contains_key(name) {
            r.resolve_winner(name, raw);
        }
    }
    cx.steps += r.steps.get();
    let mut found = r.notes.take();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, raw, code) in found {
        cx.note(notes, &name, &raw, code);
    }
    let mut map = (**parent).clone();
    cx.steps += map.len();
    for (name, entry) in r.done.take() {
        match entry {
            Entry::Value(v) => map.insert(name, Some(v)),
            Entry::Invalid => map.insert(name, None),
            Entry::Absent => map.remove(&name),
        };
    }
    Arc::new(map)
}
