//! The round trip (E4.4, slice 8): an exported frame is imported back, the imported file is laid out by the core with the
//! measurer the original was, and every box and every field the export writes is compared with the original's. The
//! harness then loads the export of the imported file in Chromium, as it does for every file.

use crate::compare;
use crate::page::{findings, imported_nodes};
use crate::table::TableMeasurer;
use crate::{Exported, Fixture};
use export::{ExportOptions, Format};
use html_import::map::{Options, plan_with};
use meldframe_layout::{Rect, compute_rects};
use meldframe_protocol::{Bind, Node, Value, Variables};
use meldframe_variables::{ModeChoice, Resolver};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// A delta that no box has: the count of nodes differs, so no box is compared.
const NO_BOXES: f64 = 1e9;

/// What came back from importing one export.
#[derive(Debug, Serialize)]
pub struct RoundTrip {
    /// The code and message when the importer refused the export.
    pub error: Option<String>,
    /// The imported file, with the `pos` the server would give each node: the harness lays it out and exports it again.
    pub nodes: Vec<Node>,
    /// The nodes of the exported frame.
    pub original: usize,
    /// The largest distance, in px, between a box of the original and the same node's box in the import.
    pub max_delta: f64,
    /// The original id of the node that is furthest off.
    pub worst: Option<String>,
    /// The fields that differ (`compare::fields`), and a count mismatch.
    pub diffs: Vec<String>,
    /// The properties the importer refused on this export, `property:code`; an export should have none.
    pub unsupported: Vec<String>,
    /// `None` when the export of the imported file is the export that was imported, with the ids renamed in order;
    /// else the first place they differ.
    pub fixed_point: Option<String>,
}

/// The frame and everything below it, a parent before its children and siblings in the order the layout uses.
fn tree_order(nodes: &[Node], root: &str) -> Vec<Node> {
    let mut out = Vec::new();
    let mut stack: Vec<&Node> = nodes.iter().filter(|n| n.id == root).collect();
    while let Some(n) = stack.pop() {
        out.push(n.clone());
        let mut kids: Vec<&Node> = nodes
            .iter()
            .filter(|k| k.parent_id.as_deref() == Some(&n.id))
            .collect();
        kids.sort_by(|a, b| {
            a.pos.cmp(&b.pos).then_with(|| {
                if a.pos.is_empty() {
                    Ordering::Equal
                } else {
                    a.id.cmp(&b.id)
                }
            })
        });
        stack.extend(kids.into_iter().rev());
    }
    out
}

/// The bindings of the original replaced by what they come to in the picked modes: the import reads the literal.
fn literal(nodes: &mut [Node], vars: &Variables, choice: &ModeChoice) {
    let r = Resolver::new(vars, choice);
    for n in nodes {
        if let Bind::Var(v) = &n.fill {
            n.fill = Bind::Lit(match r.value(&v.var) {
                Some(Value::Color(c)) => c,
                _ => "#3f3f46".into(),
            });
        }
        for b in [&mut n.radius, &mut n.opacity] {
            let id = match b {
                Bind::Var(v) => v.var.clone(),
                Bind::Lit(_) => continue,
            };
            if let Some(Value::Number(x)) = r.value(&id) {
                *b = Bind::Lit(x);
            }
        }
    }
}

/// The ids of an export renamed `n0`, `n1` ... in the order they first appear in the markup.
fn rename(text: &str, ids: &[String]) -> String {
    let mut out = text.to_owned();
    for (i, id) in ids.iter().enumerate() {
        out = out.replace(
            &format!("data-node=\"{id}\""),
            &format!("data-node=\"@{i}\""),
        );
    }
    out.replace("data-node=\"@", "data-node=\"n")
}

fn ids_in(html: &str) -> Vec<String> {
    html.split("data-node=\"")
        .skip(1)
        .filter_map(|s| s.split('"').next())
        .map(str::to_owned)
        .collect()
}

fn first_difference(a: &str, b: &str) -> Option<String> {
    let at = a
        .bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .or((a.len() != b.len()).then(|| a.len().min(b.len())))?;
    let cut = |s: &str| {
        s.chars()
            .skip(at.saturating_sub(40))
            .take(100)
            .collect::<String>()
    };
    Some(format!("at byte {at}: {:?} against {:?}", cut(a), cut(b)))
}

/// Import `exported` (the export of `frame` of `f` in `format`) and compare it with the original.
pub fn run(
    f: &Fixture,
    frame: &str,
    format: Format,
    exported: &Exported,
    rects: &BTreeMap<String, Rect>,
    measurer: &TableMeasurer,
) -> RoundTrip {
    let css = if format == Format::Tailwind {
        exported.css.as_str()
    } else {
        ""
    };
    let mut rt = RoundTrip {
        error: None,
        nodes: Vec::new(),
        original: 0,
        max_delta: 0.0,
        worst: None,
        diffs: Vec::new(),
        unsupported: Vec::new(),
        fixed_point: None,
    };
    let plan = match plan_with(&exported.html, css, &Options::default(), measurer) {
        Ok(p) => p,
        Err(e) => {
            rt.error = Some(format!("{}: {e}", e.code()));
            return rt;
        }
    };
    rt.nodes = imported_nodes(&plan);
    rt.unsupported = findings(&plan).0;
    let choice = ModeChoice::of(f.modes.iter().map(|(c, m)| (c.as_str(), m.as_str())));
    let mut original = tree_order(&f.nodes, frame);
    rt.original = original.len();
    literal(&mut original, &f.variables, &choice);
    rt.diffs = compare::fields(&original, &rt.nodes);
    if original.len() != rt.nodes.len() {
        rt.max_delta = NO_BOXES;
        return rt;
    }
    let back: BTreeMap<String, Rect> = compute_rects(&rt.nodes, measurer).into_iter().collect();
    for (o, n) in original.iter().zip(&rt.nodes) {
        let (Some(a), Some(b)) = (rects.get(&o.id), back.get(&n.id)) else {
            continue;
        };
        let d = [(a.x - b.x), (a.y - b.y), (a.w - b.w), (a.h - b.h)]
            .iter()
            .fold(0.0_f64, |m, v| m.max(v.abs()));
        if d > rt.max_delta {
            (rt.max_delta, rt.worst) = (d, Some(o.id.clone()));
        }
    }
    let Some(root) = rt.nodes.first() else {
        return rt;
    };
    let options = ExportOptions::new(format, measurer);
    if let Ok(again) = export::export_frame(&rt.nodes, &Variables::default(), &root.id, &options) {
        let (a, b) = (
            rename(&exported.html, &ids_in(&exported.html)),
            rename(&again.html, &ids_in(&again.html)),
        );
        rt.fixed_point = first_difference(&a, &b);
    }
    rt
}
