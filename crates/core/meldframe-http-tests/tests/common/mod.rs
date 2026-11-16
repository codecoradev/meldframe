//! Shared helpers: a fake submitter that records calls, a real server behind the same trait, a request builder.
#![allow(dead_code, reason = "each test binary uses a part of these helpers")]

use axum::{Router, body::Body, http::Request};
use meldframe_host::{Accepted, Op, Principal};
use meldframe_http::{Authenticator, Config, DevTokens, Failure, Rate, Submitter, router};
use meldframe_protocol::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const TOKEN: &str = "dev-token-alpha-1";
pub const OTHER: &str = "dev-token-beta-02";

pub fn tokens() -> Arc<dyn Authenticator> {
    Arc::new(
        DevTokens::parse(&format!("{TOKEN}:ws_1:agent:alpha,{OTHER}:ws_2:agent:beta")).unwrap(),
    )
}

/// Counts calls and returns what it was told to; may sleep or panic to model a slow or broken store.
pub struct Fake {
    pub calls: AtomicUsize,
    pub reply: Mutex<Result<Vec<Accepted>, Failure>>,
    pub delay_ms: AtomicU64,
    pub panics: bool,
}

impl Fake {
    pub fn ok() -> Arc<Fake> {
        Arc::new(Fake {
            calls: AtomicUsize::new(0),
            reply: Mutex::new(Ok(vec![Accepted {
                seq: 1,
                written: true,
            }])),
            delay_ms: AtomicU64::new(0),
            panics: false,
        })
    }
    pub fn failing(f: Failure) -> Arc<Fake> {
        Arc::new(Fake {
            reply: Mutex::new(Err(f)),
            ..Arc::into_inner(Fake::ok()).unwrap()
        })
    }
    pub fn slow(delay: Duration) -> Arc<Fake> {
        let f = Fake::ok();
        f.set_delay(delay);
        f
    }
    pub fn set_delay(&self, d: Duration) {
        self.delay_ms
            .store(u64::try_from(d.as_millis()).unwrap(), Ordering::SeqCst);
    }
    pub fn broken() -> Arc<Fake> {
        Arc::new(Fake {
            panics: true,
            ..Arc::into_inner(Fake::ok()).unwrap()
        })
    }
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Submitter for Fake {
    fn submit(
        &self,
        _: &str,
        _: &str,
        _: &Principal,
        _: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(self.delay_ms.load(Ordering::SeqCst)));
        assert!(!(self.panics && n == 0), "the store blew up");
        self.reply.lock().unwrap().clone()
    }
}

/// A clock the test moves by hand, in milliseconds.
pub fn manual_clock() -> (Arc<AtomicU64>, Arc<dyn Fn() -> u64 + Send + Sync>) {
    let t = Arc::new(AtomicU64::new(1_000));
    let t2 = t.clone();
    (t, Arc::new(move || t2.load(Ordering::SeqCst)))
}

pub fn config() -> Config {
    Config {
        per_client: Rate {
            burst: 1000,
            per_second: 1000,
        },
        per_principal: Rate {
            burst: 1000,
            per_second: 1000,
        },
        ..Config::default()
    }
}

pub fn app(s: Arc<dyn Submitter>, c: Config) -> Router {
    router(tokens(), s, c)
}

pub fn add(key: &str, id: &str) -> serde_json::Value {
    let node = serde_json::from_value(serde_json::json!({"id": id, "type": "frame"})).unwrap();
    let command = Command::AddNode { node, index: None };
    serde_json::json!({"key": key, "command": serde_json::to_value(command).unwrap()})
}

pub fn body(ops: Vec<serde_json::Value>) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "ops": ops })).unwrap()
}

pub fn post(tool: &str, token: Option<&str>, payload: Vec<u8>) -> Request<Body> {
    let mut r = Request::post(format!("/v1/files/file_a/tools/{tool}"))
        .header("content-type", "application/json");
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    r.body(Body::from(payload)).unwrap()
}

pub async fn json(res: axum::response::Response) -> (u16, serde_json::Value) {
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}
