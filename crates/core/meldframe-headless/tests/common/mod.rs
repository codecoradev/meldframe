//! The files a script may open, held in the test: no file system is read, so the tests run under WASM too.
#![allow(dead_code)]

use meldframe_headless::{Report, run};

pub const SCENARIO: &str = include_str!("../scripts/scenario.script");

/// The files the scripts open. `nodes.json`: a page, a frame on it and a rectangle in the frame. `flow.json`: a column of three
/// rectangles and a free frame with a rectangle, for the pointer flow.
pub fn files(name: &str) -> Result<String, String> {
    match name {
        "nodes.json" => Ok(include_str!("../scripts/nodes.json").to_string()),
        "flow.json" => Ok(include_str!("../scripts/flow.json").to_string()),
        other => Err(format!("{other}: no such file")),
    }
}

/// Run a script that starts with `open nodes.json`.
pub fn go(steps: &str, strict: bool) -> Report {
    run(&format!("open nodes.json\n{steps}"), &files, strict)
}

pub fn node(r: &Report, id: &str) -> serde_json::Value {
    let nodes = r.shell.as_ref().expect("a file is open").nodes();
    nodes
        .as_array()
        .and_then(|l| l.iter().find(|n| n["id"] == id))
        .cloned()
        .unwrap_or_else(|| panic!("no node {id}"))
}
