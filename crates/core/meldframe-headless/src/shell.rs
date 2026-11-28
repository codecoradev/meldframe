//! The shell of the harness: one [`App`] and what a platform would do for it. It carries out the effects that
//! stand for a server (it numbers each op it is sent and sends it back as the event that confirms it) and the
//! effects that stand for a screen (it answers a request to draw with the size it was asked for, and a delivery
//! with success), and it records what the app returned. It draws no pixel and holds no editor rule: a rule it
//! would need is a gap in the app layer, to be written there.

use meldframe_app::{
    Action, App, Effect, Event, Frame, PNG_MIN_BYTES, PaletteEntry, PaletteRan, Wire,
};
use meldframe_protocol::Node;
use serde_json::{Value, json};
use std::collections::VecDeque;

/// The actor the harness edits as.
pub const ACTOR: &str = "user:headless";

/// What one or more actions came to: every effect the app returned and every event it raised, in order.
#[derive(Debug, Default)]
pub struct Seen {
    pub effects: Vec<Effect>,
    pub events: Vec<Event>,
}

impl Seen {
    /// The codes of the refusals raised.
    pub fn refusals(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::Refused { code } => Some(code.clone()),
                _ => None,
            })
            .collect()
    }

    /// The kind of each effect and each event, as the app names it in its JSON shape.
    pub fn tags(items: impl IntoIterator<Item = Value>) -> Vec<String> {
        items
            .into_iter()
            .map(|v| v["type"].as_str().unwrap_or("?").to_string())
            .collect()
    }

    pub fn effect_tags(&self) -> Vec<String> {
        Self::tags(
            self.effects
                .iter()
                .filter_map(|e| serde_json::to_value(e).ok()),
        )
    }

    pub fn event_tags(&self) -> Vec<String> {
        Self::tags(
            self.events
                .iter()
                .filter_map(|e| serde_json::to_value(e).ok()),
        )
    }
}

/// An app and the platform the harness plays for it.
#[derive(Debug)]
pub struct Shell {
    app: App,
    seq: u64,
    /// The size each request to draw asked for, until its delivery.
    drawn: Vec<(u32, u32, u32)>,
    /// The size of the last image delivered.
    delivered: Option<(u32, u32)>,
}

impl Shell {
    /// Load a file and connect: the app has no server to subscribe to, so the harness says it is connected.
    pub fn open(nodes: Vec<Node>) -> Result<Shell, String> {
        let app = App::new(ACTOR, "h0", 1, nodes, 0).map_err(|e| format!("cannot load: {e:?}"))?;
        let mut s = Shell {
            app,
            seq: 0,
            drawn: Vec::new(),
            delivered: None,
        };
        s.act(Action::Connected { now_ms: 0 });
        Ok(s)
    }

    pub fn app(&self) -> &App {
        &self.app
    }

    /// The size, in pixels, of the last image the harness delivered.
    pub fn delivered(&self) -> Option<(u32, u32)> {
        self.delivered
    }

    /// The state as JSON: what a front end that is not Rust reads.
    pub fn state(&self) -> Value {
        serde_json::to_value(self.app.state()).unwrap_or(Value::Null)
    }

    /// The nodes of the file as the person sees it, as JSON.
    pub fn nodes(&self) -> Value {
        serde_json::to_value(self.app.nodes()).unwrap_or(Value::Null)
    }

    /// Dispatch an action and carry out its effects, and the actions they lead to, until nothing is left.
    pub fn act(&mut self, action: Action) -> Seen {
        let mut seen = Seen::default();
        let fx = self.app.dispatch(action);
        self.settle(fx, &mut seen);
        seen
    }

    /// The palette entries for a query, best first.
    pub fn palette(&self, query: &str) -> Vec<PaletteEntry> {
        self.app.palette(query, false)
    }

    /// Run a palette entry. `None` when the app refuses it (no such entry, or it cannot run now).
    pub fn run_palette(&mut self, id: &str, centre: (f64, f64)) -> Option<Seen> {
        let PaletteRan::Done(fx) = self.app.run_palette(id, centre) else {
            return None;
        };
        let mut seen = Seen::default();
        self.settle(fx, &mut seen);
        Some(seen)
    }

    fn settle(&mut self, first: Vec<Effect>, seen: &mut Seen) {
        let mut todo = VecDeque::from([first]);
        while let Some(effects) = todo.pop_front() {
            seen.events.extend(self.app.take_events());
            for e in effects {
                let back = self.answer(&e);
                seen.effects.push(e);
                for action in back {
                    let fx = self.app.dispatch(action);
                    todo.push_back(fx);
                }
            }
        }
        seen.events.extend(self.app.take_events());
    }

    /// What a platform sends back for an effect. An effect that needs a route the harness does not have (the lists
    /// of batches, assets and checkpoints, the clipboard, a reconnect) is recorded and left unanswered.
    fn answer(&mut self, effect: &Effect) -> Vec<Action> {
        match effect {
            Effect::SendOps { ops } => ops
                .iter()
                .map(|o| {
                    self.seq += 1;
                    Action::Frame {
                        now_ms: 0,
                        frame: Frame::Event {
                            seq: self.seq,
                            actor: ACTOR.into(),
                            key: Some(o.key.clone()),
                            command: o.command.clone(),
                        },
                    }
                })
                .collect(),
            Effect::Raster {
                request,
                width,
                height,
                ..
            } => {
                self.drawn.push((*request, *width, *height));
                let body = json!({"width": width, "height": height, "bytes": PNG_MIN_BYTES});
                let wire = Wire::Drawn {
                    request: *request,
                    status: 200,
                    body: body.to_string(),
                };
                vec![Action::Wire { wire }]
            }
            // The app dropped the image: it is not held any more.
            Effect::FinishRaster {
                request,
                deliver: false,
            } => {
                self.drawn.retain(|d| d.0 != *request);
                Vec::new()
            }
            Effect::FinishRaster {
                request,
                deliver: true,
            } => {
                if let Some((_, w, h)) = self.drawn.iter().find(|d| d.0 == *request) {
                    self.delivered = Some((*w, *h));
                }
                self.drawn.retain(|d| d.0 != *request);
                let wire = Wire::Delivered {
                    request: *request,
                    status: 200,
                    body: String::new(),
                };
                vec![Action::Wire { wire }]
            }
            _ => Vec::new(),
        }
    }
}
