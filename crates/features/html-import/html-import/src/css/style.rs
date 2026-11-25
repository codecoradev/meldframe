//! The computed style of every element: the stage after the cascade (`css::cascade`). For each element, in the order
//! of the text (a parent comes before its children), it takes the inherited properties of the parent, puts the
//! defaults table (`css::defaults`) and the cascade's declarations over them in order, and resolves what it must:
//!
//! 1. the custom properties (`--x`) first, over the parent's, so that every `var()` of the element sees all of them;
//! 2. each other declaration in turn: `var()` replaced by the literal (`css::vars`), a shorthand expanded to its
//!    longhands (`css::shorthand`), then the value put in the element's properties, so a later declaration of a
//!    property replaces an earlier one. `inherit`, `initial`, `unset` and `revert` are read for what they are
//!    (`revert` is read as `unset`: there is no user origin);
//! 3. `color`, `font-size` and `font-weight` resolved to numbers against the parent's (`em` is read against the
//!    parent's size for `font-size` itself, `currentcolor` is the parent's colour).
//!
//! The top-level elements of a fragment inherit from the virtual `body` (which inherits from the virtual `html`)
//! that the cascade made for them. Work is bounded: one step for each declaration applied and one for each custom
//! property copied, with the budget [`Limits::max_work`] for this stage; past it the elements left get the
//! inherited values only, and the report says so.

use super::cascade::{Candidate, Cascade, Matched, Origin, cascade};
use super::computed::{Custom, MAX_NOTES, Note, Prop, Props, Source, Style};
use super::defaults::{inherits, initial, ua};
use super::shorthand::expand;
use super::value::{colour, length};
use super::vars::resolve;
use super::{EntryKind, Limits, Report, read};
use crate::html::{Element, ElementId, Tree};
use std::sync::Arc;

/// The computed styles of a tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Styles {
    /// Indexed by [`ElementId::index`]; the root, at 0, has the style above everything.
    pub elements: Vec<Style>,
    /// The `html` and `body` a fragment has no element for (the style of the elements that are there is theirs).
    pub html: Option<Style>,
    pub body: Option<Style>,
    /// False when the budget of this stage ran out.
    pub complete: bool,
}

impl Styles {
    pub fn get(&self, id: ElementId) -> &Style {
        &self.elements[id.index()]
    }
}

struct Cx<'a> {
    l: &'a Limits,
    steps: usize,
    omitted: usize,
}

impl Cx<'_> {
    fn note(&mut self, notes: &mut Vec<Note>, property: &str, value: &str, code: &'static str) {
        if notes.len() >= MAX_NOTES {
            self.omitted += 1;
            return;
        }
        notes.push(Note {
            property: property.chars().take(80).collect(),
            value: value.chars().take(80).collect(),
            code,
        });
    }
}

fn has_var(v: &str) -> bool {
    v.as_bytes()
        .windows(4)
        .any(|w| w.eq_ignore_ascii_case(b"var("))
}

/// Put `value` as the value of `name`, reading the CSS-wide keywords.
fn put(
    props: &mut Props,
    parent: Option<&Style>,
    name: &Arc<str>,
    value: &str,
    source: Source,
    important: bool,
) {
    let v = value.trim();
    let inherited = inherits(name);
    let wide = v.to_ascii_lowercase();
    let from_parent = |props: &mut Props| match parent.and_then(|p| p.props.get(name)) {
        Some(p) if inherited => {
            props.insert(
                name.clone(),
                Prop {
                    source: Source::Inherited,
                    ..p.clone()
                },
            );
        }
        Some(p) => {
            props.insert(
                name.clone(),
                Prop {
                    value: p.value.clone(),
                    source,
                    important,
                },
            );
        }
        None => {
            props.remove(name);
        }
    };
    match wide.as_str() {
        "inherit" => from_parent(props),
        "unset" | "revert" if inherited => from_parent(props),
        "unset" | "revert" => {
            props.remove(name);
        }
        "initial" => {
            props.remove(name);
            if let Some(i) = initial(name).filter(|_| inherited) {
                props.insert(
                    name.clone(),
                    Prop {
                        value: i.into(),
                        source,
                        important,
                    },
                );
            }
        }
        _ => {
            props.insert(
                name.clone(),
                Prop {
                    value: v.into(),
                    source,
                    important,
                },
            );
        }
    }
}

fn apply(
    cx: &mut Cx,
    props: &mut Props,
    notes: &mut Vec<Note>,
    custom: &Custom,
    parent: Option<&Style>,
    c: &Candidate,
) {
    cx.steps += 1;
    let source = match c.origin {
        Origin::UserAgent => Source::UserAgent,
        Origin::Author => Source::Author,
        Origin::Inline => Source::Inline,
    };
    let value = if has_var(&c.value) {
        match resolve(&c.value, &|n| custom.get(n).cloned(), cx.l.max_value_bytes) {
            Ok(v) => v,
            Err(e) => return cx.note(notes, &c.name, &c.value, e.code()),
        }
    } else {
        c.value.to_string()
    };
    match expand(&c.name, &value) {
        None => put(props, parent, &c.name, &value, source, c.important),
        Some(Ok(list)) => {
            for (n, v) in list {
                put(props, parent, &Arc::from(n), &v, source, c.important);
            }
        }
        Some(Err(r)) => cx.note(notes, &c.name, &value, r.0),
    }
}

/// The text of a property that this element declared (not inherited).
fn declared<'a>(props: &'a Props, name: &str) -> Option<&'a str> {
    props
        .get(name)
        .filter(|p| p.source != Source::Inherited)
        .map(|p| p.value.trim())
}

fn typed(cx: &mut Cx, s: &mut Style, parent: &Style) {
    s.color = parent.color.clone();
    s.font_size = parent.font_size;
    s.font_weight = parent.font_weight;
    if let Some(v) = declared(&s.props, "color").map(str::to_owned) {
        // `currentcolor` is the colour the element inherits.
        if !v.eq_ignore_ascii_case("currentcolor") {
            match colour(&v) {
                Some(h) => s.color = h.hex,
                None => cx.note(&mut s.notes, "color", &v, "color_unreadable"),
            }
        }
    }
    if let Some(v) = declared(&s.props, "font-size").map(str::to_owned) {
        match length(&v, parent.font_size).filter(|l| l.px >= 0.0) {
            Some(l) => s.font_size = l.px,
            None => cx.note(&mut s.notes, "font-size", &v, "font_size_unreadable"),
        }
    }
    if let Some(v) = declared(&s.props, "font-weight").map(str::to_owned) {
        let n = match v.to_ascii_lowercase().as_str() {
            "normal" => Some(400.0),
            "bold" => Some(700.0),
            n => n.parse::<f64>().ok().filter(|n| (1.0..=1000.0).contains(n)),
        };
        match n {
            Some(n) => s.font_weight = n,
            None => cx.note(&mut s.notes, "font-weight", &v, "font_weight_unreadable"),
        }
    }
}

fn pseudo(
    cx: &mut Cx,
    list: &[Candidate],
    custom: &Custom,
    notes: &mut Vec<Note>,
) -> Option<Props> {
    if list.is_empty() {
        return None;
    }
    let mut props = Props::new();
    for c in list {
        apply(cx, &mut props, notes, custom, None, c);
    }
    Some(props)
}

/// The style of one element (`el` is `None` for a virtual one that has no defaults).
fn style_of(cx: &mut Cx, parent: &Style, el: Option<&Element>, m: &Matched) -> Style {
    let mut s = Style::root();
    s.props = parent
        .props
        .iter()
        .filter(|(k, _)| inherits(k))
        .map(|(k, p)| {
            (
                k.clone(),
                Prop {
                    source: Source::Inherited,
                    ..p.clone()
                },
            )
        })
        .collect();
    s.custom = parent.custom.clone();
    let over = cx.steps > cx.l.max_work;
    let defaults: Vec<Candidate> = match el {
        Some(e) if !over => ua(e)
            .into_iter()
            .map(|(n, v)| Candidate {
                name: n.into(),
                value: v.into(),
                important: false,
                origin: Origin::UserAgent,
            })
            .collect(),
        _ => Vec::new(),
    };
    let all: Vec<&Candidate> = if over {
        Vec::new()
    } else {
        defaults.iter().chain(&m.main).collect()
    };
    if all.iter().any(|c| c.name.starts_with("--")) {
        let mut map = (*parent.custom).clone();
        cx.steps += map.len();
        for c in all.iter().filter(|c| c.name.starts_with("--")) {
            map.insert(c.name.clone(), c.value.clone());
        }
        s.custom = Arc::new(map);
    }
    for c in all.iter().filter(|c| !c.name.starts_with("--")) {
        apply(
            cx,
            &mut s.props,
            &mut s.notes,
            &s.custom.clone(),
            Some(parent),
            c,
        );
    }
    typed(cx, &mut s, parent);
    if !over {
        s.before = pseudo(cx, &m.before, &s.custom.clone(), &mut s.notes);
        s.after = pseudo(cx, &m.after, &s.custom.clone(), &mut s.notes);
    }
    s
}

/// Compute the style of every element of `tree` from its cascade. Never fails.
pub fn compute(tree: &Tree, c: &Cascade, limits: &Limits, report: &mut Report) -> Styles {
    let virtual_el = |tag: &str| Element {
        tag: tag.into(),
        attrs: Vec::new(),
        children: Vec::new(),
        parent: tree.root_id(),
        at: 0,
    };
    let mut cx = Cx {
        l: limits,
        steps: 0,
        omitted: 0,
    };
    let top = Style::root();
    let mut out = Styles {
        elements: vec![Style::root(); tree.len() + 1],
        html: None,
        body: None,
        complete: true,
    };
    if let Some(m) = &c.html {
        out.html = Some(style_of(&mut cx, &top, Some(&virtual_el("html")), m));
    }
    if let Some(m) = &c.body {
        let parent = out.html.as_ref().unwrap_or(&top);
        out.body = Some(style_of(&mut cx, parent, Some(&virtual_el("body")), m));
    }
    let mut cut = false;
    for id in tree.ids() {
        let el = tree.element(id);
        let parent = if el.parent != tree.root_id() {
            &out.elements[el.parent.index()]
        } else {
            out.body.as_ref().or(out.html.as_ref()).unwrap_or(&top)
        };
        if cx.steps > limits.max_work && !cut {
            cut = true;
            let msg = format!(
                "computing styles took more than {} steps and stops at byte {}: the elements from there on only inherit",
                limits.max_work, el.at
            );
            report.note(EntryKind::Cut, "style_work_limit", "", msg, el.at);
        }
        let s = style_of(&mut cx, parent, Some(el), &c.elements[id.index()]);
        out.elements[id.index()] = s;
    }
    out.complete = c.complete && !cut;
    if cx.omitted > 0 {
        report.count("note_omitted", cx.omitted);
    }
    report.work += cx.steps;
    out
}

/// Read the `<style>` texts of `tree` and `extra_css`, cascade, and compute: the whole CSS side in one call.
pub fn style_tree(tree: &Tree, extra_css: &str, limits: &Limits, report: &mut Report) -> Styles {
    let sheet = read(tree.style_texts().chain([extra_css]), limits, report);
    let c = cascade(tree, &sheet, limits, report);
    compute(tree, &c, limits, report)
}
