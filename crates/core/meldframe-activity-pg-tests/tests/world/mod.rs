//! The harness of the note tests: the real server behind the MCP router, a submitter that can outlast the time limit,
//! and a reader with `batch_status` denied.
#![allow(dead_code, reason = "each test binary uses a part of these helpers")]

#[path = "../../../meldframe-mcp-tests/tests/common/mod.rs"]
mod common;
pub use activity_store::{BatchStore, MemoryBatches, Query};
pub use common::*;
use meldframe_host::{Accepted, Op, Principal};
use meldframe_http::mcp::{McpConfig, Reader};
use meldframe_http::{Config, DevTokens, Failure, Guard, Submitter};
use meldframe_http_activity::{Activity, Publish};
pub use meldframe_module::Policy;
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

/// A submitter with two faults the tests switch on. `slow`: write, then outlast the call's time limit (the caller gets
/// `timeout`, the ops are written all the same, and `finished` counts the job's end so a test waits for that and not
/// for a time). `cut`: an atomic call writes its first `cut` ops and then fails, as a store that fails part way does
/// (`Unavailable`, or `Permanent` when `permanent` is set).
pub struct Slow {
    pub inner: Arc<SharedServer>,
    pub slow: AtomicBool,
    pub finished: AtomicUsize,
    pub cut: AtomicUsize,
    pub permanent: AtomicBool,
}

impl Slow {
    fn nap(&self) {
        if self.slow.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(700));
            self.finished.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Wait until more than `before` slow jobs have ended (at most 10 seconds).
    pub async fn ended_after(&self, before: usize) {
        for _ in 0..1000 {
            if self.finished.load(Ordering::SeqCst) > before {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the slow job did not end");
    }
}

impl Submitter for Slow {
    fn submit(
        &self,
        t: &str,
        f: &str,
        w: &Principal,
        o: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure> {
        let done = self.inner.submit(t, f, w, o);
        self.nap();
        done
    }
    fn submit_atomic(
        &self,
        t: &str,
        f: &str,
        w: &Principal,
        o: Vec<Op>,
        g: Vec<Guard>,
    ) -> Result<Vec<Accepted>, Failure> {
        let cut = self.cut.load(Ordering::SeqCst);
        if cut > 0 {
            self.inner.submit(t, f, w, o[..cut].to_vec())?;
            return Err(if self.permanent.load(Ordering::SeqCst) {
                Failure::Permanent("test".into())
            } else {
                Failure::Unavailable("test".into())
            });
        }
        let done = self.inner.submit_atomic(t, f, w, o, g);
        self.nap();
        done
    }
}

/// The reader with `batch_status` denied: `applied` refuses, everything else is the server's.
pub struct NoApplied(Arc<SharedServer>);

impl Reader for NoApplied {
    fn view(&self, w: &Principal, f: &str) -> Result<meldframe_http::mcp::FileView, Failure> {
        self.0.view(w, f)
    }
    fn variables(
        &self,
        w: &Principal,
        f: &str,
    ) -> Result<(u64, meldframe_protocol::Variables), Failure> {
        self.0.variables(w, f)
    }
    fn tree(
        &self,
        w: &Principal,
        f: &str,
        n: usize,
    ) -> Result<(meldframe_http::mcp::FileView, Vec<meldframe_host::StoredOp>), Failure> {
        self.0.tree(w, f, n)
    }
    fn conflict(
        &self,
        w: &Principal,
        f: &str,
        s: u64,
        n: &str,
        l: usize,
    ) -> Result<meldframe_http::mcp::Conflict, Failure> {
        self.0.conflict(w, f, s, n, l)
    }
    fn applied(&self, _: &Principal, _: &str, _: &[String]) -> Result<Vec<Option<u64>>, Failure> {
        Err(Failure::Refused(meldframe_host::Refusal::Forbidden {
            tier: meldframe_module::RiskTier::Read,
            policy: meldframe_module::Policy::Deny,
        }))
    }
    fn written(&self, w: &Principal, f: &str, k: &[String]) -> Result<Vec<u64>, Failure> {
        self.0.written(w, f, k)
    }
    fn record(&self, w: &Principal, t: &str, f: &str, c: Option<&str>) {
        self.0.record(w, t, f, c);
    }
}

/// The store as the tool reads it, with the headers of a few key prefixes rewritten the way a row written outside the
/// server could look: `tamper-actor` gets a hidden character in its actor, `tamper-prefix` in its key prefix and
/// `tamper-by` in the name of its reviewer. Every other header passes as it is.
pub struct Tamper(pub Arc<MemoryBatches>);

impl BatchStore for Tamper {
    fn record(
        &self,
        w: &str,
        f: &str,
        b: &meldframe_activity::NewBatch,
    ) -> Result<activity_store::Recorded, activity_store::ActivityError> {
        self.0.record(w, f, b)
    }
    fn list(
        &self,
        w: &str,
        f: &str,
        q: &Query,
    ) -> Result<activity_store::Page, activity_store::ActivityError> {
        let mut page = self.0.list(w, f, q)?;
        for h in &mut page.headers {
            match h.key_prefix.split('-').take(2).collect::<Vec<_>>()[..] {
                ["tamper", "actor"] => h.actor.push('\u{200B}'),
                ["tamper", "prefix"] => h.key_prefix.push('\u{200B}'),
                ["tamper", "by"] => {
                    h.reviewed_by = Some("x\u{200B}".into());
                    h.reviewed_at_ms = Some(1);
                }
                _ => {}
            }
        }
        Ok(page)
    }
    fn set_review(
        &self,
        w: &str,
        f: &str,
        n: u32,
        s: meldframe_activity::Review,
        by: &str,
        at: u64,
    ) -> Result<activity_store::Reviewed, activity_store::ActivityError> {
        self.0.set_review(w, f, n, s, by, at)
    }
}

pub struct World {
    pub h: Harness,
    pub store: Arc<MemoryBatches>,
    pub server: Arc<SharedServer>,
    pub slow: Arc<Slow>,
}

pub async fn start(keep_notes: bool, deny_applied: bool) -> World {
    start_with(keep_notes, deny_applied, None).await
}

/// As [`start`]; `batches` is `Some(policy)` to serve `get_batches` with the registry's policy for it (it needs
/// `keep_notes`), the way the server wires it.
pub async fn start_with(
    keep_notes: bool,
    deny_applied: bool,
    batches: Option<Option<Policy>>,
) -> World {
    start_review(keep_notes, deny_applied, batches, None).await
}

/// Tokens of ws_1 with actors that a prefix rule would misjudge, for the review setter's tests.
pub const ODD: [(&str, &str); 5] = [
    ("odd-token-claude-001", "claude"),
    ("odd-token-bigagent-1", "Agent:x"),
    ("odd-token-bot-00001", "bot:x"),
    ("odd-token-person-001", "person:x"),
    ("odd-token-agentx-001", "agent:x"),
];

/// As [`start_with`]; `review` is `Some((reviewers, policy))` to serve the review setter with them.
pub async fn start_review(
    keep_notes: bool,
    deny_applied: bool,
    batches: Option<Option<Policy>>,
    review: Option<(meldframe_http_activity::Reviewers, Option<Policy>)>,
) -> World {
    let server = Arc::new(SharedServer::new(Server::assemble()));
    let slow = Arc::new(Slow {
        inner: server.clone(),
        slow: AtomicBool::new(false),
        finished: AtomicUsize::new(0),
        cut: AtomicUsize::new(0),
        permanent: AtomicBool::new(false),
    });
    let store = Arc::new(MemoryBatches::default());
    let reader: Arc<dyn Reader> = if deny_applied {
        Arc::new(NoApplied(server.clone()))
    } else {
        server.clone()
    };
    let odd: String = ODD.iter().map(|(t, a)| format!(",{t}:ws_1:{a}")).collect();
    let spec =
        format!("{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:agent:gamma{odd}");
    let tokens = Arc::new(DevTokens::parse(&spec).unwrap());
    let cfg = Config {
        timeout: Duration::from_millis(400),
        ..config()
    };
    let state = meldframe_http::shared(tokens, slow.clone(), cfg);
    let mut app = meldframe_http::routes(state.clone());
    if keep_notes {
        let live = server.clone();
        let publish: Publish = Arc::new(move |w, f, m| live.publish_with_headroom(w, f, m));
        let mut activity = Activity::with_live(store.clone(), reader.clone(), Some(publish));
        if let Some((who, policy)) = review {
            activity = activity.with_review(who, policy);
        }
        assert!(state.batches.set(activity.sink()).is_ok());
        app = app.merge(activity.routes(state.clone()));
    }
    let extra = batches.and_then(|policy| {
        meldframe_http_activity::tool(state.clone(), Arc::new(Tamper(store.clone())), policy)
    });
    let mcp = McpConfig {
        extra: extra.into_iter().collect(),
        ..McpConfig::default()
    };
    let app = app.merge(meldframe_http::mcp::routes(state, reader, mcp));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    // The spy of the shared harness is not used: the server is the one above.
    let spy = Arc::new(Spy {
        server: server.clone(),
        submits: Default::default(),
        atomic_submits: Default::default(),
        reads: Default::default(),
    });
    World {
        h: Harness { addr, spy },
        store,
        server,
        slow,
    }
}

pub fn headers(w: &World, ws: &str, file: &str) -> Vec<meldframe_activity::Header> {
    let q = Query {
        before: None,
        after: None,
        limit: 100,
    };
    w.store.list(ws, file, &q).unwrap().headers
}

pub fn apply(file: &str, key: &str, cmds: Vec<Value>, extra: Value) -> Value {
    let mut a = json!({"file": file, "idempotencyKey": key, "commands": cmds});
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}
