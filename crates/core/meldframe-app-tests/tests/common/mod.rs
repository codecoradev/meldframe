//! Shared helpers: a small file and a loopback that plays the server for one app.
#![allow(dead_code)]

use meldframe_app::{Action, App, Effect, Event, Frame};
use meldframe_protocol::{Command, Node};

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
                            command: o.command.clone(),
                        },
                    });
                }
            }
        }
        self.events.extend(self.app.take_events());
        effects
    }
}
