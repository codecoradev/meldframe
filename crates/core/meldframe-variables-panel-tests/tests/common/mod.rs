//! Helpers of the panel tests.
#![allow(dead_code)]

use meldframe_protocol::Command;
use serde_json::{Value, json};

pub fn cmd(v: Value) -> Command {
    serde_json::from_value(v).unwrap()
}

/// The collection `t` (appearance: light, dark).
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
