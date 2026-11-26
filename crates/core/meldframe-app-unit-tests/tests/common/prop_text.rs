//! The file and the helpers of the tests of typed text to a property (`property_text.rs`, `property_text_edges.rs`).

use super::{Loopback, ids};
use meldframe_app::{Action, Event, Properties, TextField, TypedText};
use meldframe_protocol::Node;

pub fn file() -> Vec<Node> {
    let n = |v: serde_json::Value| -> Node { serde_json::from_value(v).unwrap() };
    vec![
        n(serde_json::json!({"id": "root", "type": "frame", "x": 0, "y": 0, "w": 400, "h": 300})),
        n(
            serde_json::json!({"id": "r1", "type": "rect", "parentId": "root", "x": 10, "y": 20, "w": 100, "h": 50,
            "radius": 4, "opacity": 1, "fill": "#ff0000",
            "border": {"width": 2, "color": "#000000", "position": "inside"},
            "shadows": [{"x": 0, "y": 4, "blur": 8, "spread": 0, "color": "#00000040"}],
            "gradient": {"kind": "linear", "angle": 90,
                "stops": [{"pos": 0, "color": "#000000"}, {"pos": 1, "color": "#ffffff"}]}}),
        ),
        n(
            serde_json::json!({"id": "t1", "type": "text", "parentId": "root", "text": "Hi", "fontSize": 16}),
        ),
        n(
            serde_json::json!({"id": "col", "type": "frame", "parentId": "root", "layout": "column", "gap": 8, "padding": 4,
            "x": 0, "y": 0, "w": 200, "h": 200}),
        ),
        n(serde_json::json!({"id": "kid", "type": "rect", "parentId": "col", "w": 40, "h": 40})),
    ]
}

/// A connected app on `file()` with `select` selected and no event kept.
pub fn live(select: &str) -> Loopback {
    let mut l = Loopback {
        app: meldframe_app::App::new("user:me", "c0", 1, file(), 0).unwrap(),
        seq: 0,
        events: vec![],
    };
    l.app.dispatch(Action::Connected { now_ms: 0 });
    l.send(Action::Select {
        ids: ids(&[select]),
    });
    l.events.clear();
    l
}

/// Types `text` into `field` of `id` and returns the events it made.
pub fn text(l: &mut Loopback, id: &str, field: TextField, text: &str) -> Vec<Event> {
    l.send(Action::Typed {
        entry: TypedText::Property {
            id: id.into(),
            field,
            text: text.into(),
        },
    });
    l.events.drain(..).collect()
}

/// The codes of the refusals among `events`.
pub fn refused(events: &[Event]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code.as_str()),
            _ => None,
        })
        .collect()
}

pub fn steps(l: &Loopback) -> usize {
    l.app.state().history.undo_steps
}

pub fn props(l: &Loopback) -> Properties {
    l.app.state().properties.clone().unwrap()
}
