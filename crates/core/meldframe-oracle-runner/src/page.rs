//! The importer in the runner: an HTML page imported and laid out (`docs/import-html.md` section 5, point 4), and the nodes of an
//! import as plain data for both modes of the oracle.

use crate::table::{Query, TableMeasurer};
use html_import::map::{Imported, Options, import};
use meldframe_layout::{Rect, compute_rects};
use meldframe_protocol::NodeKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A hand-written page the harness sends: the HTML as it is loaded in the browser, and the width of the page.
#[derive(Debug, Deserialize)]
pub struct ImportCase {
    pub name: String,
    pub html: String,
    /// The width of the root frame and of the browser's viewport; absent, the importer's own 1440.
    #[serde(default)]
    pub width: Option<f64>,
}

/// A node of an import: the element it came from and what the importer could not take.
#[derive(Debug, Serialize)]
pub struct PageNode {
    pub id: String,
    /// `tag.class#n`, the place of the element in the text (`docs/import-html.md` section 0, item 11).
    pub source: String,
    pub kind: String,
    pub name: String,
    /// The properties changed to the nearest the model holds.
    pub approximated: Vec<String>,
    /// `code:property` of each property that was not carried.
    pub unsupported: Vec<String>,
}

/// The answer for one page.
#[derive(Debug, Serialize)]
pub struct ImportAnswer {
    pub name: String,
    pub nodes: Vec<PageNode>,
    /// The layout core's box of each imported node, with the page at the origin.
    pub rects: BTreeMap<String, Rect>,
    pub misses: Vec<Query>,
    /// The code of the refusal when the import made nothing.
    pub error: Option<String>,
}

/// The kind of a node as the file spells it.
pub(crate) fn kind(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Frame => "frame",
        NodeKind::Rect => "rect",
        NodeKind::Ellipse => "ellipse",
        NodeKind::Text => "text",
        NodeKind::Shader => "shader",
    }
}

pub(crate) fn info(i: &Imported) -> Vec<PageNode> {
    i.nodes
        .iter()
        .map(|n| PageNode {
            id: n.id.clone(),
            source: n.source.clone(),
            kind: kind(n.kind).to_owned(),
            name: n.name.clone(),
            approximated: n.approximated.iter().map(|c| c.property.clone()).collect(),
            unsupported: n
                .unsupported
                .iter()
                .map(|l| format!("{}:{}", l.code, l.property))
                .collect(),
        })
        .collect()
}

/// Import one page and lay the nodes out with the table measurer.
pub(crate) fn answer(c: &ImportCase, table: &[crate::table::Entry]) -> ImportAnswer {
    let measurer = TableMeasurer::new(table);
    let mut options = Options::default();
    if let Some(w) = c.width {
        options.width = w;
    }
    let mut a = ImportAnswer {
        name: c.name.clone(),
        nodes: Vec::new(),
        rects: BTreeMap::new(),
        misses: Vec::new(),
        error: None,
    };
    match import(&c.html, "", &options) {
        Ok(i) => {
            a.rects = compute_rects(&i.added_nodes(), &measurer)
                .into_iter()
                .collect();
            a.nodes = info(&i);
        }
        Err(e) => a.error = Some(e.code().to_owned()),
    }
    a.misses = measurer.misses();
    a
}
