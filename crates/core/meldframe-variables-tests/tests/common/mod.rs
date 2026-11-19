//! Helpers shared by the variable tests.
#![allow(dead_code)]

use meldframe_protocol::{Bind, Command, Node, NodePatch};
use serde_json::{Value, json};

pub fn cmd(v: Value) -> Command {
    serde_json::from_value(v).unwrap()
}

pub fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

pub fn add(id: &str, kind: &str, parent: Option<&str>) -> Command {
    Command::AddNode {
        node: node(id, kind, parent),
        index: None,
    }
}

/// An update that binds the fill of node `id` to variable `var`.
pub fn bind_fill(id: &str, var: &str) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            fill: Some(Bind::var(var)),
            ..Default::default()
        },
    }
}

/// The collection `t` (appearance: light, dark) of every test.
pub fn appearance() -> Command {
    cmd(
        json!({"type": "addCollection", "collection": {"id": "t", "name": "appearance",
        "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}}),
    )
}

/// The colour variable `bg` of `t`: white in light, black in dark.
pub fn bg() -> Command {
    cmd(
        json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
        "values": {"light": "#ffffff", "dark": "#000000"}}}),
    )
}

/// Set the value of `bg` in `mode`.
pub fn set_bg(mode: &str, v: &str) -> Command {
    cmd(json!({"type": "setVariableValue", "id": "bg", "mode": mode, "value": v}))
}
