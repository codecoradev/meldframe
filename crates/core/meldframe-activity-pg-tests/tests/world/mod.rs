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
use meldframe_http_activity::Activity;
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

pub struct World {
    pub h: Harness,
    pub store: Arc<MemoryBatches>,
    pub server: Arc<SharedServer>,
    pub slow: Arc<Slow>,
}

pub async fn start(keep_notes: bool, deny_applied: bool) -> World {
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
    let tokens = Arc::new(
        DevTokens::parse(&format!(
            "{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:agent:gamma"
        ))
        .unwrap(),
    );
    let cfg = Config {
        timeout: Duration::from_millis(400),
        ..config()
    };
    let state = meldframe_http::shared(tokens, slow.clone(), cfg);
    if keep_notes {
        let sink = Activity::new(store.clone(), reader.clone()).sink();
        assert!(state.batches.set(sink).is_ok());
    }
    let app = meldframe_http::routes(state.clone()).merge(meldframe_http::mcp::routes(
        state,
        reader,
        McpConfig::default(),
    ));
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
