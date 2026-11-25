//! Helpers of the mapping tests: plan a text and look at the nodes it makes.
#![allow(dead_code)]

use html_import::map::{Options, Plan, plan};
use meldframe_protocol::{Command, Node};

/// The page of a fragment with the browser's body margin taken away, so that a test sees the elements and not the
/// wrapper that margin makes.
pub const RESET: &str = "body{margin:0}";

/// Plan `html` with `css`, which must succeed.
pub fn planned(html: &str, css: &str) -> Plan {
    plan(html, css, &Options::default()).expect("plan")
}

/// The nodes the Commands add, in order.
pub fn nodes(p: &Plan) -> Vec<Node> {
    p.commands
        .iter()
        .map(|c| match c {
            Command::AddNode { node, .. } => node.clone(),
            other => panic!("not an addNode: {other:?}"),
        })
        .collect()
}

/// Plan with the body margin reset and return the nodes.
pub fn map(html: &str) -> Vec<Node> {
    nodes(&planned(html, RESET))
}

/// The node called `name`.
pub fn named<'a>(list: &'a [Node], name: &str) -> &'a Node {
    list.iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node named {name}: {:?}", names(list)))
}

pub fn names(list: &[Node]) -> Vec<&str> {
    list.iter().map(|n| n.name.as_str()).collect()
}

/// How often the report counted `code`.
pub fn count(p: &Plan, code: &str) -> usize {
    p.report.counts.get(code).copied().unwrap_or(0)
}

/// Apply the Commands one by one to an empty file with the checks every write passes, and say what is refused.
pub fn check_all(p: &Plan) -> Result<(), String> {
    let limits = meldframe_check::Limits::default();
    let mut file: Vec<meldframe_protocol::Node> = Vec::new();
    for (i, c) in p.commands.iter().enumerate() {
        meldframe_check::check_command(&file, c, &limits)
            .map_err(|e| format!("command {i}: {e:?}"))?;
        if let Command::AddNode { node, .. } = c {
            file.push(node.clone());
        }
    }
    Ok(())
}
