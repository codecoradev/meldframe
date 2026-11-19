//! Helpers of the export tests: nodes from JSON, one call to the export, and a look at one rule.
#![allow(dead_code)]

use export::{Export, ExportOptions, Format};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::{Node, Variables};
use meldframe_render::ModeChoice;
use serde_json::{Value, json};

pub fn nodes(list: Value) -> Vec<Node> {
    serde_json::from_value(list).unwrap()
}

pub fn vars(v: Value) -> Variables {
    serde_json::from_value(v).unwrap()
}

pub fn run(nodes: &[Node], vars: &Variables, format: Format, modes: ModeChoice) -> Export {
    let options = ExportOptions::new(format, &StubMeasurer).modes(modes);
    export::export_frame(nodes, vars, "f", &options).unwrap()
}

/// The plain CSS export of frame `f` of `list`, with no variables.
pub fn css(list: Value) -> Export {
    run(
        &nodes(list),
        &Variables::default(),
        Format::Css,
        ModeChoice::default(),
    )
}

/// The Tailwind export of frame `f` of `list`, with no variables.
pub fn tailwind(list: Value) -> Export {
    run(
        &nodes(list),
        &Variables::default(),
        Format::Tailwind,
        ModeChoice::default(),
    )
}

/// The declarations of the rule of node `id` in a plain CSS export (`[data-node="id"]{...}`), or `None`.
pub fn rule<'a>(e: &'a Export, id: &str) -> Option<&'a str> {
    let head = format!("[data-node=\"{id}\"]{{");
    let at = e.css.find(&head)? + head.len();
    Some(&e.css[at..at + e.css[at..].find('}')?])
}

/// The declarations of the `::after` rule of node `id`.
pub fn after<'a>(e: &'a Export, id: &str) -> Option<&'a str> {
    let head = format!("[data-node=\"{id}\"]::after{{");
    let at = e.css.find(&head)? + head.len();
    Some(&e.css[at..at + e.css[at..].find('}')?])
}

/// The classes of the element of node `id` in a Tailwind export.
pub fn classes<'a>(e: &'a Export, id: &str) -> Vec<&'a str> {
    let head = format!("data-node=\"{id}\"");
    let at = e.html.find(&head).expect("the node is in the html");
    let tag = &e.html[at..at + e.html[at..].find('>').unwrap()];
    let c = tag.split("class=\"").nth(1).expect("the node has classes");
    c.split('"').next().unwrap().split(' ').collect()
}

/// A frame `f` of 400 x 200 with the given children (JSON objects); it has no layout unless `extra` sets one.
pub fn frame(extra: Value, children: Vec<Value>) -> Value {
    let mut f = json!({"id": "f", "type": "frame", "x": 10, "y": 20, "w": 400, "h": 200});
    f.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let mut all = vec![f];
    for (i, mut c) in children.into_iter().enumerate() {
        let o = c.as_object_mut().unwrap();
        o.insert("parentId".into(), json!("f"));
        o.insert("pos".into(), json!(format!("a{i}")));
        all.push(c);
    }
    Value::Array(all)
}
