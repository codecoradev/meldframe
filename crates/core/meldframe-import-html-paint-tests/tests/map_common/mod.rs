//! Helpers of the mapping tests: import HTML and look at the nodes it made.
#![allow(dead_code)]

use html_import::map::{Imported, NodeInfo, Options, import};
use meldframe_protocol::{Command, Node, NodeKind, SizeMode};

/// The page margin of a fragment is the browser's 8 px; most tests ask for none so the numbers are the element's.
pub const FLAT: &str = "body{margin:0}";

pub fn run(html: &str) -> Imported {
    run_css(html, FLAT)
}

pub fn run_css(html: &str, css: &str) -> Imported {
    import(html, css, &Options::default()).unwrap_or_else(|e| panic!("{e}"))
}

pub fn refused(html: &str) -> html_import::map::ImportError {
    import(html, FLAT, &Options::default()).unwrap_err()
}

pub fn nodes(i: &Imported) -> Vec<Node> {
    i.commands
        .iter()
        .map(|c| match c {
            Command::AddNode { node, .. } => node.clone(),
            other => panic!("not an addNode: {other:?}"),
        })
        .collect()
}

/// The nodes of the import of `html` (with the flat page).
pub fn import_nodes(html: &str) -> Vec<Node> {
    nodes(&run(html))
}

/// The children of `id`, in order.
pub fn kids<'a>(ns: &'a [Node], id: &str) -> Vec<&'a Node> {
    ns.iter()
        .filter(|n| n.parent_id.as_deref() == Some(id))
        .collect()
}

pub fn named<'a>(ns: &'a [Node], name: &str) -> &'a Node {
    ns.iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node named {name}: {:?}", names(ns)))
}

pub fn names(ns: &[Node]) -> Vec<&str> {
    ns.iter().map(|n| n.name.as_str()).collect()
}

pub fn info<'a>(i: &'a Imported, name: &str) -> &'a NodeInfo {
    i.nodes
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node named {name}"))
}

pub fn fill(n: &Node) -> &str {
    n.fill.lit().expect("a literal fill")
}

pub fn modes(n: &Node) -> (SizeMode, SizeMode) {
    (n.sw, n.sh)
}

pub fn kind_of(n: &Node) -> NodeKind {
    n.kind
}

/// `property` of the unsupported list of the node named `name`.
pub fn lost(i: &Imported, name: &str) -> Vec<String> {
    info(i, name)
        .unsupported
        .iter()
        .map(|l| l.property.clone())
        .collect()
}

pub fn approx(i: &Imported, name: &str) -> Vec<String> {
    info(i, name)
        .approximated
        .iter()
        .map(|c| c.property.clone())
        .collect()
}
