//! Hand-written pages (E4.4, slice 8): the importer's reading of a page, laid out by the layout core with the same
//! measurer as every other fixture. The harness loads the page in Chromium and compares the boxes it makes with
//! these rects, node by node, through the node's name (`data-name`).

use crate::table::{Entry, Query, TableMeasurer};
use html_import::map::{Options, Plan, plan_with};
use meldframe_layout::{Rect, compute_rects};
use meldframe_protocol::{Command, Node};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A page to import: the HTML, the extra CSS the call would carry, and the width of the root when the page gives none.
#[derive(Debug, Deserialize)]
pub struct Page {
    pub name: String,
    pub html: String,
    #[serde(default)]
    pub css: String,
    #[serde(default)]
    pub width: Option<f64>,
}

/// The import of a page.
#[derive(Debug, Serialize)]
pub struct PageAnswer {
    pub name: String,
    /// The code and message when the importer refused the text.
    pub error: Option<String>,
    /// The imported file: the nodes the Commands add, each with the `pos` the server would give it.
    pub nodes: Vec<Node>,
    /// The core's box of every imported node.
    pub rects: BTreeMap<String, Rect>,
    /// The element each node came from, by node id.
    pub sources: BTreeMap<String, String>,
    /// The properties the importer refused on any node, `property:code`, each once.
    pub unsupported: Vec<String>,
    /// The properties it carried only in part, each once.
    pub approximated: Vec<String>,
    /// How often each code of the report happened.
    pub report: BTreeMap<String, usize>,
    /// What the table lacked in this run, as for a fixture.
    pub misses: Vec<Query>,
}

/// The nodes a plan adds, in order, each with a `pos` that keeps the order of the text, as the server gives one to an
/// `addNode` that has none. The pos strings are the oracle's own (a sibling's index, six digits): the importer does not
/// choose them and the contract does not compare them (the losses of `docs/import-html.md` section 3.7).
pub(crate) fn imported_nodes(plan: &Plan) -> Vec<Node> {
    let mut next: BTreeMap<Option<String>, usize> = BTreeMap::new();
    plan.commands
        .iter()
        .filter_map(|c| match c {
            Command::AddNode { node, .. } => Some(node.clone()),
            _ => None,
        })
        .map(|mut n| {
            let k = next.entry(n.parent_id.clone()).or_insert(0);
            n.pos = format!("{k:06}");
            *k += 1;
            n
        })
        .collect()
}

/// The properties the importer refused and approximated on the nodes of a plan, each once, in order.
pub(crate) fn findings(plan: &Plan) -> (Vec<String>, Vec<String>) {
    let (mut refused, mut approx) = (BTreeSet::new(), BTreeSet::new());
    for n in &plan.nodes {
        refused.extend(
            n.unsupported
                .iter()
                .map(|u| format!("{}:{}", u.property, u.code)),
        );
        approx.extend(n.approximated.iter().map(|a| a.property.clone()));
    }
    (refused.into_iter().collect(), approx.into_iter().collect())
}

pub fn answer(p: &Page, table: &[Entry]) -> PageAnswer {
    let measurer = TableMeasurer::new(table);
    let options = Options {
        width: p.width.unwrap_or(1440.0),
        ..Options::default()
    };
    let mut a = PageAnswer {
        name: p.name.clone(),
        error: None,
        nodes: Vec::new(),
        rects: BTreeMap::new(),
        sources: BTreeMap::new(),
        unsupported: Vec::new(),
        approximated: Vec::new(),
        report: BTreeMap::new(),
        misses: Vec::new(),
    };
    match plan_with(&p.html, &p.css, &options, &measurer) {
        Err(e) => a.error = Some(format!("{}: {e}", e.code())),
        Ok(plan) => {
            a.nodes = imported_nodes(&plan);
            a.rects = compute_rects(&a.nodes, &measurer).into_iter().collect();
            a.sources = plan
                .nodes
                .iter()
                .map(|n| (n.id.clone(), n.source.clone()))
                .collect();
            (a.unsupported, a.approximated) = findings(&plan);
            a.report = plan
                .report
                .counts
                .iter()
                .map(|(k, v)| ((*k).to_owned(), *v))
                .collect();
        }
    }
    a.misses = measurer.misses();
    a
}
