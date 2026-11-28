//! M1 exit criteria 3, 4 and 5 without a web UI: a headless editor (the app layer of `meldframe-app`) on the
//! live WebSocket of the real composed server, an rmcp agent on `/mcp`, and the capture relay.

mod common;
use common::*;
use meldframe_app::{Action, App, Effect, Frame};
use meldframe_host::Principal;
use meldframe_http::mcp::Reader;
use meldframe_protocol::Command;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;

const FILE: &str = "landing";
const ACTOR: &str = "user:bea";

/// The shell of one headless editor: it carries out the app's effects over the real transports (REST for ops,
/// the live socket for events) and holds no editor rule.
struct Person<'a> {
    h: &'a Harness,
    app: App,
    socket: Option<Editor>,
    /// Numbers of the events received on the current socket, in order.
    received: Vec<u64>,
    capture_requests: usize,
}

impl<'a> Person<'a> {
    async fn open(h: &'a Harness) -> Person<'a> {
        let app = App::new(ACTOR, "ed", 1, vec![], 0).unwrap();
        let mut p = Person {
            h,
            app,
            socket: None,
            received: vec![],
            capture_requests: 0,
        };
        p.act(Action::Start { now_ms: 0 }).await;
        p
    }

    fn seq(&self) -> u64 {
        self.app.state().confirmed_seq
    }

    fn frame(v: &Value) -> Option<Action> {
        let frame = match v["type"].as_str()? {
            "event" => Frame::Event {
                seq: v["seq"].as_u64()?,
                actor: v["actor"].as_str()?.to_string(),
                key: v["key"].as_str().map(str::to_string),
                command: serde_json::from_value::<Command>(v["command"].clone()).ok()?,
            },
            "heartbeat" => Frame::Heartbeat {},
            "resync" => Frame::Resync {
                after_seq: v["afterSeq"].as_u64()?,
                latest_seq: v["latestSeq"].as_u64()?,
            },
            _ => return None,
        };
        Some(Action::Frame { now_ms: 0, frame })
    }

    /// Dispatch an action, carry out its effects and the actions they lead to.
    async fn act(&mut self, action: Action) {
        let mut queue = std::collections::VecDeque::from([action]);
        while let Some(action) = queue.pop_front() {
            for effect in self.app.dispatch(action) {
                queue.extend(self.perform(&effect).await);
            }
        }
    }

    async fn perform(&mut self, effect: &Effect) -> Vec<Action> {
        match effect {
            Effect::SendOps { ops } => {
                let mut back = vec![];
                for o in ops {
                    let r = reqwest::Client::new()
                        .post(format!(
                            "http://{}/v1/files/{FILE}/tools/apply_commands",
                            self.h.addr
                        ))
                        .bearer_auth(PERSON)
                        .json(
                            &json!({"ops": [{"key": o.key, "command": o.command, "deps": o.deps}]}),
                        )
                        .send()
                        .await
                        .unwrap();
                    if r.status().as_u16() != 200 {
                        let body: Value = r.json().await.unwrap();
                        back.push(Action::Frame {
                            now_ms: 0,
                            frame: Frame::Rejected {
                                key: o.key.clone(),
                                code: body["error"]["code"].as_str().unwrap_or("?").into(),
                                retryable: false,
                            },
                        });
                    }
                }
                back
            }
            Effect::Subscribe { after_seq } => {
                let e =
                    Editor::open_after(self.h, PERSON, FILE, &["capture"], Some(*after_seq)).await;
                self.received.clear();
                let mut back = vec![Action::Connected { now_ms: 0 }];
                for v in &e.early {
                    self.note(v);
                    back.extend(Self::frame(v));
                }
                self.socket = Some(e);
                back
            }
            _ => vec![],
        }
    }

    fn note(&mut self, v: &Value) {
        if v["type"] == "event" {
            self.received.push(v["seq"].as_u64().unwrap());
        }
    }

    /// Read the socket until the editor has applied everything up to `seq`.
    async fn sync_to(&mut self, seq: u64) {
        while self.seq() < seq {
            let v = self.socket.as_mut().expect("connected").next().await;
            self.note(&v);
            if let Some(a) = Self::frame(&v) {
                self.act(a).await;
            }
        }
    }

    /// Nothing more arrives for a moment.
    async fn assert_quiet(&mut self) {
        let v = self
            .socket
            .as_mut()
            .unwrap()
            .try_next(Duration::from_millis(400))
            .await;
        assert!(v.is_none(), "unexpected frame {v:?}");
    }

    /// Serve the next capture request like an editor that renders: answer with a PNG, tagged with the number it has seen.
    async fn answer_capture(&mut self) -> Value {
        loop {
            let v = self.socket.as_mut().expect("connected").next().await;
            if v["type"] == "captureRequest" {
                self.capture_requests += 1;
                let token = v["uploadToken"].as_str().unwrap().to_string();
                let status = upload(self.h, PERSON, &v, &token, self.seq(), valid_png()).await;
                assert_eq!(status, 200);
                return v;
            }
            self.note(&v);
            if let Some(a) = Self::frame(&v) {
                self.act(a).await;
            }
        }
    }

    fn snapshot(&self) -> String {
        snap(self.app.nodes())
    }
}

fn snap(nodes: &[meldframe_protocol::Node]) -> String {
    let mut v = nodes.to_vec();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&v).unwrap()
}

fn body(r: &rmcp::model::CallToolResult) -> Value {
    assert_ne!(r.is_error, Some(true), "{r:?}");
    r.structured_content.clone().expect("a structured body")
}

fn add(id: &str, kind: &str, parent: Option<&str>) -> Value {
    json!({"type": "addNode", "node": {"id": id, "type": kind, "name": id, "parentId": parent,
        "x": 10, "y": 20, "w": 100, "h": 30}})
}

async fn write(a: &Agent, key: &str, commands: Vec<Value>) -> Value {
    body(
        &call(
            a,
            "apply_commands",
            json!({"file": FILE, "idempotencyKey": key, "commands": commands}),
        )
        .await,
    )
}

async fn position(a: &Agent, id: &str) -> (Option<f64>, Option<f64>) {
    let r = body(&call(a, "get_nodes", json!({"file": FILE, "ids": [id]})).await);
    (r["nodes"][0]["x"].as_f64(), r["nodes"][0]["y"].as_f64())
}

async fn ids(a: &Agent) -> Vec<String> {
    let r = body(
        &call(
            a,
            "get_nodes",
            json!({"file": FILE, "ids": ["hero", "heading", "blurb", "badge", "n9", "n10"]}),
        )
        .await,
    );
    let mut v: Vec<String> = r["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn undo_touches_only_the_persons_change_a_reconnect_replays_the_gap_and_the_agent_captures() {
    let h = start().await;
    let agent = agent(&h, AGENT).await;
    let mut person = Person::open(&h).await;
    assert_eq!(
        person.app.state().connection,
        meldframe_app::Connection::Live
    );

    // The agent builds the hero in one batch; the open editor shows it without a reload.
    let hero = write(
        &agent,
        "hero-1",
        vec![
            add("hero", "frame", None),
            add("heading", "text", Some("hero")),
            add("blurb", "text", Some("hero")),
        ],
    )
    .await;
    assert_eq!(hero["seq"], 3);
    person.sync_to(3).await;
    assert_eq!(person.app.nodes().len(), 3);

    // 3. The person moves the heading, the agent adds a node after that, the person presses undo.
    person
        .act(Action::Move {
            id: "heading".into(),
            x: 40.0,
            y: 60.0,
        })
        .await;
    person.sync_to(4).await;
    assert_eq!(position(&agent, "heading").await, (Some(40.0), Some(60.0)));
    assert_eq!(
        write(&agent, "badge-1", vec![add("badge", "frame", Some("hero"))]).await["seq"],
        5
    );
    person.sync_to(5).await;
    assert!(person.app.state().history.can_undo);
    person.act(Action::Undo {}).await;
    person.sync_to(6).await;
    assert!(
        !person.app.state().history.can_undo,
        "only the person's move was on the stack"
    );
    // Only the person's move is undone; the agent's nodes remain, and the agent's next read shows it.
    assert_eq!(position(&agent, "heading").await, (Some(10.0), Some(20.0)));
    assert_eq!(ids(&agent).await, ["badge", "blurb", "heading", "hero"]);
    // A second undo has nothing of the person's to undo and writes nothing.
    person.act(Action::Undo {}).await;
    person.assert_quiet().await;
    assert_eq!(
        h.server
            .view(&Principal::new("ws_1", ACTOR), FILE)
            .unwrap()
            .seq,
        6
    );

    // 4. The socket drops; the agent writes meanwhile; the editor reconnects with its last number.
    drop(person.socket.take());
    person.act(Action::Disconnected {}).await;
    assert_eq!(
        person.app.state().connection,
        meldframe_app::Connection::Offline
    );
    let gap = write(
        &agent,
        "gap-1",
        vec![
            add("n9", "rect", Some("hero")),
            add("n10", "rect", Some("hero")),
        ],
    )
    .await;
    assert_eq!(gap["seq"], 8);
    assert_eq!(person.seq(), 6, "the editor missed the agent's write");
    person.act(Action::Reconnect { now_ms: 2_000 }).await;
    person.sync_to(8).await;
    assert_eq!(
        person.received,
        [7, 8],
        "exactly the missed changes, nothing before them"
    );
    person.assert_quiet().await;
    let view = h.server.view(&Principal::new("ws_1", ACTOR), FILE).unwrap();
    assert_eq!(
        person.snapshot(),
        snap(&view.nodes),
        "the editor converged to the server"
    );
    assert!(person.app.state().pending.is_empty());

    // 5. The agent captures the frame; the editor (the only capable client) answers with a PNG.
    let asker = tokio::spawn(async move {
        call(&agent, "capture", json!({"file": FILE, "nodeId": "hero"})).await
    });
    let req = person.answer_capture().await;
    assert_eq!(req["nodeId"], "hero");
    assert_eq!(person.capture_requests, 1);
    assert_eq!(image(&asker.await.unwrap()), valid_png());

    // 6. Every write is in the log with its actor (the log has no decision column: see the report).
    let (_, log) = h
        .server
        .tree(&Principal::new("ws_1", ACTOR), FILE, 1000)
        .unwrap();
    let mut per_actor: BTreeMap<String, usize> = BTreeMap::new();
    for op in &log {
        assert!(!op.actor.is_empty() && !op.key.is_empty(), "{op:?}");
        *per_actor.entry(op.actor.clone()).or_default() += 1;
    }
    eprintln!("log entries per actor: {per_actor:?}");
    assert_eq!(
        per_actor,
        BTreeMap::from([("agent:alpha".to_string(), 6), (ACTOR.to_string(), 2)])
    );
    assert_eq!(log.len() as u64, view.seq);
}
