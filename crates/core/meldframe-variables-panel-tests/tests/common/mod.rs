//! Helpers of the panel tests.
#![allow(dead_code)]

use meldframe_app::{Action, App, BindField, BindOption, Event, Panel, VariableRow, VariableValue};
use meldframe_protocol::{Command, Doc, Node};
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

pub fn round() -> meldframe_protocol::Command {
    cmd(
        json!({"type": "addVariable", "variable": {"id": "round", "collection": "t", "name": "radius/card", "type": "number",
        "values": {"light": 4, "dark": 12}}}),
    )
}

/// A frame `card` with its fill bound to `bg`, a plain rectangle and a text node; `bg` and `round` in `t`.
pub fn file() -> Doc {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "card", "type": "rect", "pos": "a0", "fill": {"var": "bg"}},
        {"id": "plain", "type": "rect", "pos": "a1"},
        {"id": "label", "type": "text", "pos": "a2"},
        {"id": "oval", "type": "ellipse", "pos": "a3"},
    ]))
    .unwrap();
    let mut d = Doc::of_nodes(nodes);
    for c in [appearance(), bg(), round()] {
        meldframe_core::apply_doc(&mut d, &c, &Default::default()).unwrap();
    }
    d
}

pub fn open() -> App {
    let mut a = App::with_doc("user:a", "t", 1, file(), 0).unwrap();
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: true,
    });
    a.take_events();
    a
}

pub fn row<'a>(a: &'a App, id: &str) -> &'a VariableRow {
    a.state()
        .variables
        .collections
        .iter()
        .flat_map(|c| &c.variables)
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no row {id}: {:?}", a.state().variables))
}

pub fn texts(r: &VariableRow) -> Vec<(&str, &str)> {
    r.values
        .iter()
        .map(|v| (v.mode.as_str(), v.text.as_str()))
        .collect()
}

pub fn select(a: &mut App, id: &str) {
    a.dispatch(Action::Select {
        ids: vec![id.into()],
    });
}

pub fn set(variable: &str, mode: &str, text: &str) -> Action {
    Action::SetVariableValue {
        variable: variable.into(),
        mode: mode.into(),
        value: VariableValue::Literal { text: text.into() },
    }
}

pub fn refused(a: &mut App) -> Vec<String> {
    a.take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect()
}

pub fn binds(a: &App, id: &str) -> Vec<BindOption> {
    row(a, id).binds.clone()
}

pub fn opt(field: BindField, bound: bool) -> BindOption {
    BindOption { field, bound }
}
