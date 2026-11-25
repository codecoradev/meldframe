//! The computed style of every element: the stage after the cascade (`css::cascade`). For each element, in the order
//! of the text (a parent comes before its children), it takes the inherited properties of the parent, puts the
//! defaults table (`css::defaults`) and the cascade's declarations over them in order, and resolves what it must:
//!
//! 1. the custom properties (`--x`) first. The last declaration of a name wins; each winner is then resolved here,
//!    on the element that declares it, against the parent's custom properties and the element's own (in any order,
//!    as CSS says), so `:root{--b:1px;--a:var(--b)}` gives `--a` the 1px of the root to every element, whatever a
//!    child says of `--b`. A winner that cannot be resolved (a name nobody has and no fallback, a cycle, too long)
//!    is the guaranteed-invalid value, which hides the parent's and lets `var(--a, fallback)` use its fallback.
//!    `initial` is that value, `inherit` and `unset` the parent's;
//! 2. each other declaration in turn: `var()` replaced by the literal (`css::vars`), a shorthand expanded to its
//!    longhands (`css::shorthand`), then the value put in the element's properties, so a later declaration of a
//!    property replaces an earlier one. A declaration whose `var()` cannot be resolved is invalid when computed and
//!    so the property is unset, whatever it had before (the browser's default below it does not come back).
//!    `inherit`, `initial` and `unset` are read for what they are; `revert` gives the property its value from the
//!    defaults table (or inherits it, when the table has none);
//! 3. `color`, `font-size` and `font-weight` resolved to numbers against the parent's (`em` is read against the
//!    parent's size for `font-size` itself, `rem` against the root's, `currentcolor` is the parent's colour).
//!
//! The top-level elements of a fragment inherit from the virtual `body` (which inherits from the virtual `html`)
//! that the cascade made for them. Work is bounded: a step for each declaration applied, for each custom property
//! copied, and for what a `var()` cost (its replacements and a step for each 64 bytes looked at), with the budget
//! [`Limits::max_work`] for this stage; past it the elements left get the inherited values only, and the report
//! says so.

use crate::computed::{Custom, MAX_NOTES, Note, Prop, Props, Source, Style};
use crate::custom::{Wide, customs, has_var, wide};
use crate::defaults::{inherits, initial, ua, ua_page};
use crate::shorthand::{expand, longhands};
use crate::value::{colour, length};
use crate::vars::{Cost, resolve};
use html_import_css::css::{
    Candidate, Cascade, EntryKind, Limits, Matched, Origin, Report, cascade, read,
};
use html_import_tree::{ElementId, Tree};
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

pub(crate) struct Cx<'a> {
    pub l: &'a Limits,
    pub steps: usize,
    /// Bytes kept: [`ENTRY_BYTES`] for each property put, and the text that was made.
    pub bytes: usize,
    pub omitted: usize,
}

/// What a property of an element is charged for the entry itself (the name's pointer, the value's pointer and the
/// source, in a tree of properties): a number, not a measure.
pub(crate) const ENTRY_BYTES: usize = 64;

impl Cx<'_> {
    pub fn note(&mut self, notes: &mut Vec<Note>, property: &str, value: &str, code: &'static str) {
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

    /// What a `var()` resolution cost: a step, its replacements, and a step for each 64 bytes it looked at.
    pub fn charge(&mut self, c: Cost) {
        self.steps += 1 + c.expansions + c.scanned / 64;
    }

    fn over(&self) -> bool {
        self.steps > self.l.max_work || self.bytes > self.l.max_style_bytes
    }
}

/// Put `value` as the value of `name`, reading the CSS-wide keywords. `ua` is what the defaults table gave the
/// element, for `revert`; `parent` is `None` for a generated box.
fn put(
    props: &mut Props,
    ua: &Props,
    parent: Option<&Style>,
    name: &Arc<str>,
    value: Arc<str>,
    source: Source,
) {
    let inherited = inherits(name);
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
                },
            );
        }
        None => {
            props.remove(name);
        }
    };
    match wide(&value) {
        Some(Wide::Inherit) => from_parent(props),
        Some(Wide::Unset) if inherited => from_parent(props),
        Some(Wide::Unset) => {
            props.remove(name);
        }
        Some(Wide::Revert) => match ua.get(name) {
            Some(p) => {
                props.insert(name.clone(), p.clone());
            }
            None if inherited => from_parent(props),
            None => {
                props.remove(name);
            }
        },
        Some(Wide::Initial) => {
            props.remove(name);
            if let Some(i) = initial(name).filter(|_| inherited) {
                props.insert(
                    name.clone(),
                    Prop {
                        value: i.into(),
                        source,
                    },
                );
            }
        }
        None => {
            props.insert(name.clone(), Prop { value, source });
        }
    }
}

/// The property, or the longhands of a shorthand, unset: what a declaration that is invalid when computed does.
fn unset(props: &mut Props, ua: &Props, parent: Option<&Style>, name: &Arc<str>, source: Source) {
    let names: Vec<Arc<str>> = match longhands(name) {
        Some(l) => l.into_iter().map(Arc::from).collect(),
        None => vec![name.clone()],
    };
    for n in names {
        put(props, ua, parent, &n, "unset".into(), source);
    }
}

#[allow(clippy::too_many_arguments)]
fn apply(
    cx: &mut Cx,
    props: &mut Props,
    ua: &Props,
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
    cx.bytes += ENTRY_BYTES;
    let value: Arc<str> = if has_var(&c.value) {
        let (r, cost) = resolve(
            &c.value,
            &|n| custom.get(n).cloned().flatten(),
            cx.l.max_value_bytes,
        );
        cx.charge(cost);
        match r {
            Ok(v) => {
                cx.bytes += v.len();
                v.into()
            }
            Err(e) => {
                cx.note(notes, &c.name, &c.value, e.code());
                return unset(props, ua, parent, &c.name, source);
            }
        }
    } else {
        c.value.clone()
    };
    match expand(&c.name, &value) {
        None => put(props, ua, parent, &c.name, value, source),
        Some(Ok(list)) => {
            for (n, v) in list {
                cx.bytes += ENTRY_BYTES + n.len() + v.len();
                put(props, ua, parent, &Arc::from(n), v.into(), source);
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
        match length(&v, parent.font_size, s.rem).filter(|l| l.px >= 0.0) {
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
        apply(cx, &mut props, &Props::new(), notes, custom, None, c);
    }
    Some(props)
}

/// Whose style it is: an element of the tree, or the `html` or the `body` of a fragment.
#[derive(Clone, Copy)]
enum Place<'a> {
    El(&'a Tree, ElementId),
    Html,
    Body,
}

/// The style of one element, `rem` the pixels of a `rem` for it.
fn style_of(cx: &mut Cx, parent: &Style, place: Place, rem: f64, m: &Matched) -> Style {
    let mut s = Style::root();
    s.rem = rem;
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
    cx.bytes += s.props.len() * ENTRY_BYTES;
    s.custom = parent.custom.clone();
    let over = cx.over();
    let table = match place {
        _ if over => Vec::new(),
        Place::El(tree, id) => ua(tree, id),
        Place::Html => ua_page(false),
        Place::Body => ua_page(true),
    };
    let defaults: Vec<Candidate> = table
        .into_iter()
        .map(|(n, v)| Candidate {
            name: n.into(),
            value: v.into(),
            important: false,
            origin: Origin::UserAgent,
        })
        .collect();
    let all: Vec<&Candidate> = if over {
        Vec::new()
    } else {
        defaults.iter().chain(&m.main).collect()
    };
    s.custom = customs(cx, &mut s.notes, parent, &all);
    // What the defaults table gave the element alone, for `revert`.
    let mut ua_props = Props::new();
    let custom = s.custom.clone();
    for c in all.iter().filter(|c| !c.name.starts_with("--")) {
        if c.origin == Origin::UserAgent {
            apply(
                cx,
                &mut ua_props,
                &Props::new(),
                &mut Vec::new(),
                &custom,
                Some(parent),
                c,
            );
        }
        apply(
            cx,
            &mut s.props,
            &ua_props,
            &mut s.notes,
            &custom,
            Some(parent),
            c,
        );
    }
    typed(cx, &mut s, parent);
    if !over {
        s.before = pseudo(cx, &m.before, &custom, &mut s.notes);
        s.after = pseudo(cx, &m.after, &custom, &mut s.notes);
    }
    s
}

/// Compute the style of every element of `tree` from its cascade. Never fails.
pub fn compute(tree: &Tree, c: &Cascade, limits: &Limits, report: &mut Report) -> Styles {
    let mut cx = Cx {
        l: limits,
        steps: 0,
        bytes: 0,
        omitted: 0,
    };
    let top = Style::root();
    let mut out = Styles {
        elements: vec![Style::root(); tree.len() + 1],
        html: None,
        body: None,
        complete: true,
    };
    // The root's font size, which every other element reads a `rem` against; the root itself reads 16.
    let mut root_rem = 16.0;
    if let Some(m) = &c.html {
        let s = style_of(&mut cx, &top, Place::Html, 16.0, m);
        root_rem = s.font_size;
        out.html = Some(s);
    }
    if let Some(m) = &c.body {
        let parent = out.html.as_ref().unwrap_or(&top);
        out.body = Some(style_of(&mut cx, parent, Place::Body, root_rem, m));
    }
    let mut cut = false;
    for id in tree.ids() {
        let el = tree.element(id);
        let parent = if el.parent != tree.root_id() {
            &out.elements[el.parent.index()]
        } else {
            out.body.as_ref().or(out.html.as_ref()).unwrap_or(&top)
        };
        if cx.over() && !cut {
            cut = true;
            let msg = format!(
                "computing styles took more than {} steps or {} bytes and stops at byte {}: the elements from there on only inherit",
                limits.max_work, limits.max_style_bytes, el.at
            );
            report.note(EntryKind::Cut, "style_work_limit", "", msg, el.at);
        }
        let is_root = el.tag == "html" && el.parent == tree.root_id();
        let rem = if is_root { 16.0 } else { root_rem };
        let s = style_of(
            &mut cx,
            parent,
            Place::El(tree, id),
            rem,
            &c.elements[id.index()],
        );
        if is_root {
            root_rem = s.font_size;
        }
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
