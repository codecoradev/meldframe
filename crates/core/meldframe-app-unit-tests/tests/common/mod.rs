//! Shared helpers: a small file and a loopback that plays the server for one app.
#![allow(dead_code)]

use meldframe_app::{Action, App, Effect, Event, Frame};
use meldframe_protocol::{Command, Node};

/// What a raw answer comes to, compared as JSON (an action has no equality of its own).
#[derive(Debug)]
pub struct Out(pub Option<Action>);

impl PartialEq for Out {
    fn eq(&self, other: &Self) -> bool {
        serde_json::to_value(&self.0).unwrap() == serde_json::to_value(&other.0).unwrap()
    }
}

pub fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

/// A root frame holding a rectangle and a text node.
pub fn doc() -> Vec<Node> {
    vec![
        node("root", "frame", None),
        node("r1", "rect", Some("root")),
        node("t1", "text", Some("root")),
    ]
}

pub fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

pub fn app() -> App {
    App::new("user:me", "c0", 1, doc(), 0).unwrap()
}

pub fn get(app: &App, id: &str) -> Node {
    app.nodes().iter().find(|n| n.id == id).cloned().unwrap()
}

/// The commands carried by the `SendOps` effects, in order.
pub fn sent(effects: &[Effect]) -> Vec<Command> {
    effects
        .iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.iter().map(|o| o.command.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

/// Plays the server for one app: numbers what it is sent and sends it back as an event frame.
pub struct Loopback {
    pub app: App,
    pub seq: u64,
    pub events: Vec<Event>,
}

impl Loopback {
    pub fn live() -> Self {
        let mut app = app();
        app.dispatch(Action::Connected { now_ms: 0 });
        Loopback {
            app,
            seq: 0,
            events: vec![],
        }
    }

    /// Dispatch, then confirm every op the effects send. Returns all effects seen.
    pub fn send(&mut self, action: Action) -> Vec<Effect> {
        let effects = self.app.dispatch(action);
        for e in &effects {
            if let Effect::SendOps { ops } = e {
                for o in ops {
                    self.seq += 1;
                    self.app.dispatch(Action::Frame {
                        now_ms: 0,
                        frame: Frame::Event {
                            seq: self.seq,
                            actor: "user:me".into(),
                            key: Some(o.key.clone()),
                            command: o.command.clone(),
                        },
                    });
                }
            }
        }
        self.events.extend(self.app.take_events());
        effects
    }

    /// An op of another actor that the server ordered next, delivered to the app.
    pub fn remote(&mut self, actor: &str, key: &str, now_ms: u64, command: Command) {
        self.seq += 1;
        self.app.dispatch(Action::Frame {
            now_ms,
            frame: Frame::Event {
                seq: self.seq,
                actor: actor.into(),
                key: Some(key.into()),
                command,
            },
        });
        self.events.extend(self.app.take_events());
    }
}

pub fn select(l: &mut Loopback, id: &str) {
    l.send(Action::Select { ids: ids(&[id]) });
}

pub fn open(l: &mut Loopback) {
    l.send(Action::OpenExport {});
}

/// The codes of the refusals seen so far (drains the events).
pub fn refused(l: &mut Loopback) -> Vec<String> {
    l.events
        .drain(..)
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect()
}

/// What a table row feeds in, what every name field must show, and what a lane (an identifier, so white space is left
/// as the server sent it) must show.
pub struct Case {
    pub what: &'static str,
    pub given: &'static str,
    pub name: &'static str,
    pub actor: &'static str,
}

pub const CASES: [Case; 10] = [
    Case {
        what: "U+2800 braille blank",
        given: "a\u{2800}b",
        name: "ab",
        actor: "agent:ab",
    },
    Case {
        what: "U+3164 Hangul filler",
        given: "a\u{3164}b",
        name: "ab",
        actor: "agent:ab",
    },
    Case {
        what: "U+17B4 Khmer vowel",
        given: "a\u{17B4}b",
        name: "ab",
        actor: "agent:ab",
    },
    Case {
        what: "U+200B zero width space",
        given: "a\u{200B}b",
        name: "ab",
        actor: "agent:ab",
    },
    Case {
        what: "U+00AD soft hyphen",
        given: "a\u{00AD}b",
        name: "ab",
        actor: "agent:ab",
    },
    Case {
        what: "two U+00A0",
        given: "a\u{00A0}\u{00A0}b",
        name: "a b",
        actor: "agent:a\u{00A0}\u{00A0}b",
    },
    Case {
        what: "U+202E override",
        given: "a\u{202E}b",
        name: "ab",
        actor: "agent:ab",
    },
    Case {
        what: "U+0007 control character",
        given: "a\u{7}b",
        name: "ab",
        actor: "agent:ab",
    },
    Case {
        what: "U+0085 next line reads as a gap, like a tab",
        given: "a\u{85}b",
        name: "a b",
        actor: "agent:ab",
    },
    Case {
        what: "U+2028 line separator reads as a gap, like a line break",
        given: "a\u{2028}b",
        name: "a b",
        actor: "agent:ab",
    },
];

pub mod prop_text;
