//! The harness of the setter tests: a server with batch 1 in file `f`, and calls to the route.
#![allow(dead_code, reason = "each test binary uses a part of these helpers")]

#[path = "../activity_world/mod.rs"]
pub mod world;
pub use meldframe_host::Principal;
use meldframe_http_activity::Reviewers;
use meldframe_http_live::LiveSource;
use meldframe_protocol::ServerMsg;
use meldframe_server::{Frame, StreamId};
use serde_json::{Value, json};
pub use world::*;

pub const PATH: &str = "/v1/files/f/batches/1/review";
pub const BEA: &str = PERSON; // user:bea
pub const CLAUDE: &str = ODD[0].0;
pub const BIG_AGENT: &str = ODD[1].0; // Agent:x
pub const BOT: &str = ODD[2].0;
pub const PERSON_X: &str = ODD[3].0;
pub const AGENT_X: &str = ODD[4].0; // agent:x
pub const EVERYONE: [&str; 8] = [AGENT, BEA, OTHER, CLAUDE, BIG_AGENT, BOT, PERSON_X, AGENT_X];

/// A server whose reviewers are `spec` and whose registry policy for the setter is `policy`, with batch 1 of
/// `agent:alpha` in file `f` of ws_1.
pub async fn served(spec: &str, policy: Option<Policy>) -> World {
    let who = Reviewers::parse(spec).unwrap();
    let w = start_review(true, false, Some(Some(Policy::Auto)), Some((who, policy))).await;
    let c = connect(&w.h, AGENT).await;
    let args = apply("f", "k1", vec![frame("a", "A")], json!({"note": "n"}));
    assert!(!call(&c, "apply_commands", args).await.is_error);
    w
}

pub async fn put_to(w: &World, token: Option<&str>, path: &str, body: &str) -> (u16, Value) {
    let url = format!("http://{}{path}", w.h.addr);
    let mut req = reqwest::Client::new()
        .put(url)
        .header("content-type", "application/json")
        .body(body.to_owned());
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let r = req.send().await.unwrap();
    (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
}

pub async fn set(w: &World, token: &str, state: &str) -> (u16, Value) {
    put_to(w, Some(token), PATH, &json!({"state": state}).to_string()).await
}

pub fn code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap_or("")
}

pub fn header(w: &World) -> meldframe_activity::Header {
    headers(w, "ws_1", "f").remove(0)
}

pub fn who(ws: &str, actor: &str) -> Principal {
    Principal::new(ws, actor)
}

pub fn batch_frames(w: &World, p: &Principal, id: StreamId) -> Vec<ServerMsg> {
    let all = w.server.poll(p, "f", id, 0, 100_000);
    let msgs = all.into_iter().filter_map(|f| match f {
        Frame::Ephemeral(m @ ServerMsg::Batch { .. }) => Some(m),
        _ => None,
    });
    msgs.collect()
}
