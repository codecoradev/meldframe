//! The shell of the tests: it plays the platform for several app instances against one real in-memory
//! server. It carries out the effects (send ops, subscribe, schedule a reconnect), turns the server's
//! frames into actions, and records what it saw. It holds no editor rule.
#![allow(dead_code, missing_debug_implementations)]

use meldframe_app::{Action, App, Effect, Event, Frame as AppFrame};
use meldframe_host::{Op, Principal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::{Command, Node};
use meldframe_server::{Config, Frame, Server, StreamId};
use std::collections::VecDeque;

pub const WS: &str = "ws_1";
pub const FILE: &str = "file_1";

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

pub fn config(queue: usize) -> Config {
    Config {
        window: 64,
        queue,
        max_streams: 8,
        heartbeat_ms: 500,
        dead_after_ms: 10_000_000,
        stall_ms: 10_000_000,
    }
}

/// One editor: an app and what the shell knows about its connection.
pub struct Client {
    pub name: String,
    pub app: App,
    stream: Option<StreamId>,
    /// Every effect the app returned, in order.
    pub effects: Vec<Effect>,
    pub events: Vec<Event>,
    /// Connection states the app went through, in order.
    pub states: Vec<meldframe_app::Connection>,
}

pub struct World {
    pub server: Server,
    pub clients: Vec<Client>,
    pub now: u64,
}

pub fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

pub fn snap(nodes: &[Node]) -> String {
    let mut v = nodes.to_vec();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&v).unwrap()
}

impl World {
    /// A server with `root` (a frame) and `t1` (a text node) already in the file.
    pub fn new(queue: usize) -> Self {
        let mut server = Server::assemble_with(vec![Box::new(Painter)]).with_live(config(queue), 8);
        let who = Principal::new(WS, "system");
        let seed = [
            node("root", "frame", None),
            node("t1", "text", Some("root")),
        ];
        for (i, n) in seed.into_iter().enumerate() {
            let cmd = Command::AddNode {
                node: n,
                index: None,
            };
            let op = Op::new(format!("seed{i}"), cmd);
            server.submit_tool("paint", FILE, &who, vec![op]).unwrap();
        }
        World {
            server,
            clients: Vec::new(),
            now: 1_000,
        }
    }

    /// The file on the server.
    pub fn room(&self) -> (Vec<Node>, u64) {
        self.server
            .with_room(WS, FILE, |r| (r.nodes().to_vec(), r.seq()))
            .unwrap()
    }

    /// A new editor that loads the file as it is now and opens its connection.
    pub fn join(&mut self, actor: &str, prefix: &str, session: u64) -> usize {
        let (nodes, seq) = self.room();
        let app = App::new(actor, prefix, session, nodes, seq).unwrap();
        self.clients.push(Client {
            name: actor.to_string(),
            app,
            stream: None,
            effects: vec![],
            events: vec![],
            states: vec![],
        });
        let i = self.clients.len() - 1;
        let now = self.now;
        self.act(i, Action::Start { now_ms: now });
        i
    }

    pub fn app(&self, i: usize) -> &App {
        &self.clients[i].app
    }

    /// Dispatch an action, then carry out its effects, and the actions they lead to, until nothing is left.
    pub fn act(&mut self, i: usize, action: Action) {
        let mut queue = VecDeque::from([action]);
        while let Some(action) = queue.pop_front() {
            let c = &mut self.clients[i];
            let effects = c.app.dispatch(action);
            c.events.extend(c.app.take_events());
            let state = c.app.state().connection;
            if c.states.last() != Some(&state) {
                c.states.push(state);
            }
            for effect in effects {
                let more = self.perform(i, &effect);
                self.clients[i].effects.push(effect);
                queue.extend(more);
            }
        }
    }

    fn perform(&mut self, i: usize, effect: &Effect) -> Vec<Action> {
        let who = Principal::new(WS, &self.clients[i].name);
        match effect {
            Effect::SendOps { ops } => {
                let mut back = vec![];
                for o in ops {
                    let op = Op::new(o.key.clone(), o.command.clone()).with_deps(o.deps.clone());
                    match self.server.submit_tool("paint", FILE, &who, vec![op]) {
                        // The answer names the seq, also for a repeated key (`written` is false).
                        Ok(done) => back.push(Action::Frame {
                            now_ms: self.now,
                            frame: AppFrame::Acked {
                                key: o.key.clone(),
                                seq: done[0].seq,
                            },
                        }),
                        Err(e) => {
                            let r = e.refusal().expect("a refusal");
                            back.push(Action::Frame {
                                now_ms: self.now,
                                frame: AppFrame::Rejected {
                                    key: o.key.clone(),
                                    code: r.code().into(),
                                    retryable: r.retryable(),
                                },
                            });
                        }
                    }
                }
                back
            }
            Effect::Subscribe { after_seq } => {
                let joined = match self.clients[i].stream {
                    None => self.server.subscribe(&who, FILE, *after_seq, self.now),
                    Some(id) => self
                        .server
                        .resubscribe(&who, FILE, id, *after_seq, self.now),
                }
                .expect("the server takes the subscription");
                let fresh = self.clients[i].stream.replace(joined.id).is_none();
                let mut back = vec![];
                if fresh {
                    back.push(Action::Connected { now_ms: self.now });
                }
                for op in joined.replay {
                    back.push(self.frame(AppFrame::Event {
                        seq: op.seq,
                        actor: op.actor,
                        key: Some(op.key),
                        command: op.command,
                    }));
                }
                back
            }
            // The batch list, the review route, the asset list and the checkpoint routes are the front end's: a test that
            // needs them plays them.
            Effect::Reconnect { .. }
            | Effect::WakeAfter { .. }
            | Effect::Render { .. }
            | Effect::FetchBatches { .. }
            | Effect::FetchAssets { .. }
            | Effect::PutBatchReview { .. }
            | Effect::FetchCheckpoints {}
            | Effect::PostCheckpoint { .. }
            | Effect::FetchCheckpointState { .. }
            | Effect::Raster { .. }
            | Effect::FinishRaster { .. }
            | Effect::FailCapture { .. } => vec![],
        }
    }

    fn frame(&self, frame: AppFrame) -> Action {
        Action::Frame {
            now_ms: self.now,
            frame,
        }
    }

    /// Deliver every frame waiting for client `i`; returns how many.
    pub fn pump(&mut self, i: usize) -> usize {
        let Some(id) = self.clients[i].stream else {
            return 0;
        };
        let who = Principal::new(WS, &self.clients[i].name);
        let mut n = 0;
        while let Some(f) = self.server.poll(&who, FILE, id, self.now) {
            let frame = match f {
                Frame::Durable { seq, data } => AppFrame::Event {
                    seq,
                    actor: data.actor,
                    key: Some(data.key),
                    command: data.command,
                },
                Frame::Resync {
                    after_seq,
                    latest_seq,
                } => AppFrame::Resync {
                    after_seq,
                    latest_seq,
                },
                Frame::Heartbeat => AppFrame::Heartbeat {},
                Frame::Ephemeral(_) => continue,
            };
            n += 1;
            let action = self.frame(frame);
            self.act(i, action);
        }
        n
    }

    pub fn pump_all(&mut self) {
        for i in 0..self.clients.len() {
            self.pump(i);
        }
    }

    /// The connection of client `i` drops: the server forgets its stream, the app is told.
    pub fn drop_link(&mut self, i: usize) {
        if let Some(id) = self.clients[i].stream.take() {
            let who = Principal::new(WS, &self.clients[i].name);
            self.server.unsubscribe(&who, FILE, id);
        }
        self.act(i, Action::Disconnected {});
    }

    /// An agent writes through the same gate, as `agent:claude`.
    pub fn agent(&mut self, commands: Vec<Command>) {
        let who = Principal::new(WS, "agent:claude");
        for c in commands {
            let key = format!("agent-{}", self.room().1 + 1);
            let op = Op::new(key, c);
            self.server
                .submit_tool("paint", FILE, &who, vec![op])
                .unwrap();
        }
    }

    /// Every client equals the file on the server and has nothing pending.
    pub fn assert_converged(&self) {
        let (nodes, seq) = self.room();
        for c in &self.clients {
            assert_eq!(snap(c.app.nodes()), snap(&nodes), "{} view", c.name);
            assert_eq!(snap(c.app.base()), snap(&nodes), "{} base", c.name);
            assert!(c.app.state().pending.is_empty(), "{} pending", c.name);
            assert_eq!(c.app.state().confirmed_seq, seq, "{} seq", c.name);
        }
    }
}
