//! Helpers of the manage tests.
#![allow(dead_code)]

use meldframe_app::{
    Action, App, Effect, Event, Frame, ManageOp, ManageTarget, NameError, NameWarning, OutOp,
    Panel, VariableEntity,
};
use meldframe_protocol::{Bind, Command, Doc, Node, Value, Variable};
use serde_json::{Value as Json, json};

pub fn cmd(v: Json) -> Command {
    serde_json::from_value(v).unwrap()
}

/// The file: the collection `t` (appearance: light, dark) with the colour `bg` (white, black) and the number `round`
/// (4, 12); the collection `u` (one mode, `base`) with the colour `accent`, an alias of `bg`; a rectangle `card` bound
/// to `bg` and `round`, a rectangle `plain` and a rectangle `dot` bound to `accent`.
pub fn file() -> Doc {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "card", "type": "rect", "pos": "a0", "fill": {"var": "bg"}, "radius": {"var": "round"}},
        {"id": "plain", "type": "rect", "pos": "a1"},
        {"id": "dot", "type": "rect", "pos": "a2", "fill": {"var": "accent"}},
    ]))
    .unwrap();
    let mut d = Doc::of_nodes(nodes);
    for c in [
        json!({"type": "addCollection", "collection": {"id": "t", "name": "appearance",
            "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}}),
        json!({"type": "addCollection", "collection": {"id": "u", "name": "brand",
            "modes": [{"id": "base", "name": "base"}]}}),
        json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t", "name": "color/bg",
            "type": "color", "values": {"light": "#ffffff", "dark": "#000000"}}}),
        json!({"type": "addVariable", "variable": {"id": "round", "collection": "t", "name": "radius/card",
            "type": "number", "values": {"light": 4, "dark": 12}}}),
        json!({"type": "addVariable", "variable": {"id": "accent", "collection": "u", "name": "color/accent",
            "type": "color", "values": {"base": {"var": "bg"}}}}),
    ] {
        meldframe_core::apply_doc(&mut d, &cmd(c), &Default::default()).unwrap();
    }
    d
}

/// An app on `file()` with the Variables panel open and the connection live.
pub fn open() -> App {
    let mut a = App::with_doc("user:a", "t", 1, file(), 0).unwrap();
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: true,
    });
    a.dispatch(Action::Start { now_ms: 0 });
    a.dispatch(Action::Connected { now_ms: 0 });
    a.take_events();
    a
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

/// The ops of the `SendOps` effects among `fx`.
pub fn sent(fx: &[Effect]) -> Vec<OutOp> {
    fx.iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.clone(),
            _ => Vec::new(),
        })
        .collect()
}

/// The server orders and confirms `ops` of `actor` from sequence number `from + 1`; returns the last number.
pub fn confirm(a: &mut App, actor: &str, ops: Vec<OutOp>, from: u64) -> u64 {
    let mut seq = from;
    for o in ops {
        seq += 1;
        a.dispatch(Action::Frame {
            now_ms: seq,
            frame: Frame::Event {
                seq,
                actor: actor.into(),
                key: Some(o.key),
                command: o.command,
            },
        });
    }
    seq
}

/// Another actor's op, ordered as event `seq`.
pub fn remote(a: &mut App, seq: u64, command: Command) {
    a.dispatch(Action::Frame {
        now_ms: seq,
        frame: Frame::Event {
            seq,
            actor: "user:b".into(),
            key: None,
            command,
        },
    });
}

pub fn node<'a>(a: &'a App, id: &str) -> &'a Node {
    a.nodes().iter().find(|n| n.id == id).unwrap()
}

pub fn variable<'a>(a: &'a App, id: &str) -> Option<&'a Variable> {
    a.vars().variable(id)
}

pub fn lit(v: &Bind<Value>) -> Value {
    v.lit().cloned().expect("a literal")
}

/// The id the last `EntityAdded` event named.
pub fn made(events: &[Event]) -> Vec<(meldframe_app::VariableEntity, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::EntityAdded { entity, id } => Some((*entity, id.clone())),
            _ => None,
        })
        .collect()
}

pub fn dark(a: &mut App) {
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    a.take_events();
}

/// The file, with the collections and variables by id: a variable that comes back is listed last, and the order of
/// the list carries no meaning.
pub fn snapshot(a: &App) -> String {
    let mut vars = a.vars().clone();
    vars.collections.sort_by(|x, y| x.id.cmp(&y.id));
    vars.variables.sort_by(|x, y| x.id.cmp(&y.id));
    format!("{:?}{vars:?}", a.nodes())
}

pub fn color(s: &str) -> Bind<Value> {
    Bind::Lit(Value::Color(s.into()))
}

pub fn op(op: ManageOp) -> Action {
    Action::Manage { op }
}

pub fn typed(a: &mut App, form: VariableEntity, text: &str) {
    a.dispatch(op(ManageOp::SetDraft {
        form,
        text: text.into(),
    }));
}

pub fn variable_target(id: &str) -> ManageTarget {
    ManageTarget::Variable { id: id.into() }
}

pub fn collection_target(id: &str) -> ManageTarget {
    ManageTarget::Collection { id: id.into() }
}

pub fn mode_target(collection: &str, id: &str) -> ManageTarget {
    ManageTarget::Mode {
        collection: collection.into(),
        id: id.into(),
    }
}

pub fn check(a: &App, form: VariableEntity) -> (Option<NameError>, Option<NameWarning>, String) {
    let d = &a.state().manage.drafts;
    let d = match form {
        VariableEntity::Collection => &d.collection,
        VariableEntity::Mode => &d.mode,
        VariableEntity::Variable => &d.variable,
    };
    (d.check.error, d.check.warning, d.check.name.clone())
}
