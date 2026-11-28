//! The cascade: for each element, the declarations that apply to it, in the order the cascade gives them, lowest
//! first. A later one wins over an earlier one of the same property, so the next stage folds the list in order.
//!
//! The order is the one of CSS for the origins this importer has: author rules and the `style` attribute (the
//! browser's own defaults are a table of the next stage, below both). From lowest: normal author declarations, by
//! specificity and then source order; normal inline declarations; `!important` author declarations, by the same;
//! `!important` inline declarations. Within a rule, declarations keep their order, and a list of selectors counts
//! once with the specificity of the most specific selector that matches.
//!
//! Rules are looked up by the id, class or type of the last compound of each selector, so an element is tested
//! only against the selectors that could pick it. The whole cascade has a budget of steps ([`Limits::max_work`]):
//! a text that would cost more (thousands of `*` rules over thousands of elements) is cut there, in the order of
//! the elements, and the report says so; the elements left get no declarations at all.
//!
//! A report entry of a `style` attribute carries the byte where its element starts plus the offset inside the value,
//! not a byte of the document: the tree keeps the start of the element and not of its attributes.

use super::matching::{Matcher, Node};
use super::selector::{Pseudo, Simple};
use super::{Decl, EntryKind, Limits, Report, Stylesheet, read_declarations};
use crate::tw;
use html_import_tree::{ElementId, MAX_ENTRIES, Tree};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Where a declaration came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A rule of a `<style>` or of the extra css.
    Author,
    /// The `style` attribute of the element.
    Inline,
    /// The browser's defaults (`css::defaults`): the cascade never makes one; the computed style puts them
    /// under everything else.
    UserAgent,
}

/// A declaration that applies to an element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: Arc<str>,
    pub value: Arc<str>,
    pub important: bool,
    pub origin: Origin,
}

/// What applies to one element, lowest first: to the element itself, and to its generated `::before` and `::after`
/// (only the declarations of rules that name them; the `style` attribute never reaches them).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Matched {
    pub main: Vec<Candidate>,
    pub before: Vec<Candidate>,
    pub after: Vec<Candidate>,
    /// The classes of the element that no rule names and that [`crate::tw`] does not read, in the order written,
    /// each once, at most [`MAX_UNRESOLVED`].
    pub unresolved: Vec<Arc<str>>,
}

/// The most unresolved classes one element lists.
pub const MAX_UNRESOLVED: usize = 32;

/// The result: one [`Matched`] for each element of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cascade {
    /// Indexed by [`html_import_tree::ElementId::index`]; the root, at 0, has nothing.
    pub elements: Vec<Matched>,
    /// For the `html` and `body` that a fragment has no element for: what applies to them (the next stage takes
    /// the custom properties and the inherited values from them). `None` when the tree has the element itself.
    pub html: Option<Matched>,
    pub body: Option<Matched>,
    /// False when the budget ran out: the elements from the one the report names on have nothing.
    pub complete: bool,
}

#[derive(Default)]
struct Index {
    by_id: HashMap<String, Vec<(u32, u32)>>,
    by_class: HashMap<String, Vec<(u32, u32)>>,
    by_tag: HashMap<String, Vec<(u32, u32)>>,
    any: Vec<(u32, u32)>,
}

/// The index of the rules, and every class a selector names.
fn index(sheet: &Stylesheet) -> (Index, HashSet<String>) {
    let mut ix = Index::default();
    let mut named = HashSet::new();
    for (r, rule) in sheet.rules.iter().enumerate() {
        for (s, sel) in rule.selectors.iter().enumerate() {
            for simple in sel.compounds.iter().flat_map(|c| &c.simples) {
                if let Simple::Class(c) = simple {
                    named.insert(c.clone());
                }
            }
            let at = (r as u32, s as u32);
            let last = &sel.compounds[sel.compounds.len() - 1].simples;
            if let Some(id) = last.iter().find_map(|s| match s {
                Simple::Id(i) => Some(i),
                _ => None,
            }) {
                ix.by_id.entry(id.clone()).or_default().push(at);
            } else if let Some(c) = last.iter().find_map(|s| match s {
                Simple::Class(c) => Some(c),
                _ => None,
            }) {
                ix.by_class.entry(c.clone()).or_default().push(at);
            } else if let Some(t) = last.iter().find_map(|s| match s {
                Simple::Tag(t) => Some(t),
                _ => None,
            }) {
                ix.by_tag.entry(t.clone()).or_default().push(at);
            } else {
                ix.any.push(at);
            }
        }
    }
    (ix, named)
}

/// One declaration with what orders it: important, origin, specificity, rule, place in the rule.
type Key = (bool, u8, u32, u32, u32);

fn fold(mut v: Vec<(Key, Candidate)>) -> Vec<Candidate> {
    v.sort_by_key(|(k, _)| *k);
    v.into_iter().map(|(_, c)| c).collect()
}

fn candidate(d: &Decl, origin: Origin) -> Candidate {
    Candidate {
        name: d.name.clone(),
        value: d.value.clone(),
        important: d.important,
        origin,
    }
}

/// A Tailwind class counts as a class selector of the last rule: more specific than a type, below an id, and after
/// every rule of the sheet when they are equal.
const CLASS_SPECIFICITY: u32 = 1 << 8;

struct Run<'a> {
    /// Every class that a selector of the sheet names: a class that is one of them belongs to the sheet.
    named: HashSet<String>,
    /// The classes already in the report.
    seen: HashSet<String>,
    tree: &'a Tree,
    sheet: &'a Stylesheet,
    ix: Index,
    m: Matcher<'a>,
    limits: &'a Limits,
    report: &'a mut Report,
}

impl Run<'_> {
    /// The selectors that could pick `n`, by what the last compound asks for.
    fn possible(&mut self, n: Node) -> Vec<(u32, u32)> {
        let (ix, m) = (&self.ix, &mut self.m);
        let mut out = Vec::new();
        // A list is charged before it is copied, and not copied once the budget is gone, so that a hostile text cannot
        // make a list of millions first and be charged after.
        let mut add = |extra: usize, l: Option<&Vec<(u32, u32)>>| {
            m.steps += extra;
            if let Some(l) = l {
                m.steps += l.len();
                if !m.exhausted() {
                    out.extend(l.iter().copied());
                }
            }
        };
        add(0, Some(&ix.any));
        match n {
            Node::Html | Node::Body => {
                let t = if n == Node::Html { "html" } else { "body" };
                add(0, ix.by_tag.get(t));
            }
            Node::El(e) => {
                let el = self.tree.element(e);
                add(0, ix.by_tag.get(&el.tag));
                if let Some(id) = el.attr("id") {
                    add(0, ix.by_id.get(id));
                }
                // Each distinct class adds its list once, however often the attribute repeats it; each word of the
                // attribute costs a step.
                let mut seen = std::collections::HashSet::new();
                for c in el.attr("class").unwrap_or("").split_ascii_whitespace() {
                    add(1, None);
                    if seen.insert(c) {
                        add(0, ix.by_class.get(c));
                    }
                }
            }
        }
        out
    }

    fn matched(&mut self, n: Node) -> Matched {
        // The rules that pick `n`, for the element and for each generated box, with their best specificity.
        let mut hits: Vec<(u32, u8, u32)> = Vec::new();
        for (r, s) in self.possible(n) {
            if self.m.exhausted() {
                break;
            }
            let sel = &self.sheet.rules[r as usize].selectors[s as usize];
            if self.m.matches(sel, n) {
                let ps = match sel.pseudo {
                    None => 0,
                    Some(Pseudo::Before) => 1,
                    Some(Pseudo::After) => 2,
                };
                hits.push((r, ps, sel.specificity));
            }
        }
        hits.sort_unstable();
        let (mut main, mut before, mut after) = (Vec::new(), Vec::new(), Vec::new());
        let mut i = 0;
        while i < hits.len() {
            // The last of a run of the same rule and box has the highest specificity.
            let (r, ps, _) = hits[i];
            while i + 1 < hits.len() && hits[i + 1].0 == r && hits[i + 1].1 == ps {
                i += 1;
            }
            let spec = hits[i].2;
            let out = match ps {
                0 => &mut main,
                1 => &mut before,
                _ => &mut after,
            };
            let rule = &self.sheet.rules[r as usize];
            self.m.steps += rule.decls.len();
            for (k, d) in rule.decls.iter().enumerate() {
                let key = (d.important, 0, spec, r, k as u32);
                out.push((key, candidate(d, Origin::Author)));
            }
            i += 1;
        }
        let unresolved = match n {
            Node::El(e) => self.classes(e, &mut main, &mut after),
            _ => Vec::new(),
        };
        if let Node::El(e) = n
            && let Some(text) = self.tree.element(e).attr("style")
        {
            let decls = read_declarations(text, self.tree.element(e).at, self.limits, self.report);
            self.m.steps += decls.len();
            for (k, d) in decls.iter().enumerate() {
                let key = (d.important, 1, 0, 0, k as u32);
                main.push((key, candidate(d, Origin::Inline)));
            }
        }
        Matched {
            main: fold(main),
            before: fold(before),
            after: fold(after),
            unresolved,
        }
    }

    /// The classes of `e` that no rule names, read as Tailwind utilities: what they say goes on the lists as
    /// author declarations with the specificity of a class, after every rule, in the order of the classes. The ones
    /// that are not read are returned, and counted in the report once for each distinct class.
    fn classes(
        &mut self,
        e: ElementId,
        main: &mut Vec<(Key, Candidate)>,
        after: &mut Vec<(Key, Candidate)>,
    ) -> Vec<Arc<str>> {
        let mut unresolved: Vec<Arc<str>> = Vec::new();
        let el = self.tree.element(e);
        let mut place = 0u32;
        for class in el.attr("class").unwrap_or("").split_ascii_whitespace() {
            if self.named.contains(class) {
                continue;
            }
            match tw::decode(class, self.limits.max_value_bytes) {
                Ok(d) => {
                    let (list, spec) = match d.pseudo {
                        Some(_) => (&mut *after, CLASS_SPECIFICITY | 1),
                        None => (&mut *main, CLASS_SPECIFICITY),
                    };
                    for (name, value) in d.decls {
                        let key = (false, 0, spec, u32::MAX, place);
                        place += 1;
                        let c = Candidate {
                            name: name.into(),
                            value: value.into(),
                            important: false,
                            origin: Origin::Author,
                        };
                        list.push((key, c));
                    }
                }
                Err(why) => {
                    if !unresolved.iter().any(|u| &**u == class)
                        && unresolved.len() < MAX_UNRESOLVED
                    {
                        unresolved.push(class.into());
                    }
                    // The first of a class is an entry (which counts it); the others are counted.
                    if self.seen.len() < MAX_ENTRIES && self.seen.insert(class.to_owned()) {
                        let msg = format!(
                            "not read as a Tailwind utility of the export ({}) and no rule names it",
                            why.0
                        );
                        self.report
                            .note(EntryKind::Listed, "class_unresolved", class, msg, el.at);
                    } else {
                        self.report.count("class_unresolved", 1);
                    }
                }
            }
        }
        unresolved
    }
}

/// Work out what applies to every element of `tree`. Never fails; what it could not do is in `report`.
pub fn cascade(tree: &Tree, sheet: &Stylesheet, limits: &Limits, report: &mut Report) -> Cascade {
    let (ix, named) = index(sheet);
    let mut run = Run {
        named,
        seen: HashSet::new(),
        tree,
        sheet,
        ix,
        m: Matcher::new(tree, limits.max_work),
        limits,
        report,
    };
    let mut out = Cascade {
        elements: vec![Matched::default(); tree.len() + 1],
        html: None,
        body: None,
        complete: true,
    };
    let mut order = Vec::new();
    if run.m.virtual_html() {
        order.push(Node::Html);
    }
    if run.m.virtual_body() {
        order.push(Node::Body);
    }
    order.extend(tree.ids().map(Node::El));
    for n in order {
        let m = run.matched(n);
        if run.m.exhausted() {
            let at = if let Node::El(e) = n {
                tree.element(e).at
            } else {
                0
            };
            let msg = format!(
                "the cascade took more than {} steps and stops at byte {at}: the elements from there on get no styles",
                limits.max_work
            );
            run.report
                .note(EntryKind::Cut, "cascade_work_limit", "", msg, at);
            out.complete = false;
            break;
        }
        match n {
            Node::Html => out.html = Some(m),
            Node::Body => out.body = Some(m),
            Node::El(e) => out.elements[e.index()] = m,
        }
    }
    run.report.work += run.m.steps;
    out
}
