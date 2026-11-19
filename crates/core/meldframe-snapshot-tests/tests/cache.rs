//! The attribution is cached per (workspace, file) and head sequence number, so a repeated read does not
//! page the log again; a write invalidates it, the cache is bounded and workspaces never share an entry.
//! A call that had to scan is charged more of the principal's budget.

use axum::{Router, body::Body, http::Request};
use meldframe_host::{
    Appended, MemoryStore, NewOp, Principal, Room, Snapshot, Store, StoreError, StoredOp,
};
use meldframe_http::{Authenticator, Config, DevTokens, Failure, Rate};
use meldframe_http_snapshot::{
    AttributionCache, MAX_CACHED_FILES, Read, Snapshots, snapshot_cached,
};
use meldframe_protocol::{Command, FileSnapshot, NodePatch};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use tower::ServiceExt;

/// A memory store that counts how often the log is paged.
struct Counting {
    inner: MemoryStore,
    paged: Arc<AtomicUsize>,
}

impl Store for Counting {
    fn append(&mut self, room: &str, op: NewOp) -> Result<Appended, StoreError> {
        self.inner.append(room, op)
    }
    fn find(&self, room: &str, actor: &str, key: &str) -> Result<Option<u64>, StoreError> {
        self.inner.find(room, actor, key)
    }
    fn latest_seq(&self, room: &str) -> Result<u64, StoreError> {
        self.inner.latest_seq(room)
    }
    fn ops_after(&self, room: &str, seq: u64, limit: usize) -> Result<Vec<StoredOp>, StoreError> {
        self.paged.fetch_add(1, SeqCst);
        self.inner.ops_after(room, seq, limit)
    }
    fn save_snapshot(&mut self, room: &str, snapshot: Snapshot) -> Result<(), StoreError> {
        self.inner.save_snapshot(room, snapshot)
    }
    fn latest_snapshot(&self, room: &str) -> Result<Option<Snapshot>, StoreError> {
        self.inner.latest_snapshot(room)
    }
}

fn store() -> (Counting, Arc<AtomicUsize>) {
    let paged = Arc::new(AtomicUsize::new(0));
    let s = Counting {
        inner: MemoryStore::default(),
        paged: paged.clone(),
    };
    (s, paged)
}

fn put(s: &mut Counting, room: &str, actor: &str, key: &str, command: Command) {
    let op = NewOp {
        actor: actor.into(),
        key: key.into(),
        command,
    };
    s.append(room, op).unwrap();
}

fn add(id: &str) -> Command {
    let node = serde_json::from_value(json!({"id": id, "type": "rect"})).unwrap();
    Command::AddNode { node, index: None }
}

fn nudge(id: &str) -> Command {
    let patch = NodePatch {
        x: Some(1.0),
        ..NodePatch::default()
    };
    Command::UpdateNode {
        id: id.into(),
        patch,
    }
}

fn actor_of(r: &Read, id: &str) -> Option<String> {
    let v = serde_json::to_value(&r.snapshot).unwrap();
    let n = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id);
    n.unwrap()["lastActor"].as_str().map(str::to_owned)
}

#[test]
fn a_second_read_at_the_same_seq_pages_nothing_and_a_write_invalidates_it() {
    let (mut s, paged) = store();
    put(&mut s, "f", "user:alice", "k1", add("n1"));
    let cache = AttributionCache::default();
    let room = Room::open("ws_1", "f", s).unwrap();
    paged.store(0, SeqCst);
    let first = snapshot_cached(&room, 5_000, &cache).unwrap().unwrap();
    assert!(first.scanned && paged.load(SeqCst) > 0);
    paged.store(0, SeqCst);
    let second = snapshot_cached(&room, 5_000, &cache).unwrap().unwrap();
    assert!(!second.scanned, "a hit does no scan");
    assert_eq!(paged.load(SeqCst), 0, "a hit pages nothing");
    assert_eq!(actor_of(&second, "n1").as_deref(), Some("user:alice"));

    // A write moves the head: the entry is stale and the answer is worked out again.
    let mut s = room.into_store();
    put(&mut s, "f", "user:bob", "k2", nudge("n1"));
    let room = Room::open("ws_1", "f", s).unwrap();
    paged.store(0, SeqCst);
    let third = snapshot_cached(&room, 5_000, &cache).unwrap().unwrap();
    assert!(third.scanned && paged.load(SeqCst) > 0);
    assert_eq!(actor_of(&third, "n1").as_deref(), Some("user:bob"));
}

#[test]
fn the_cache_is_bounded_and_forgets_the_oldest_file_first() {
    let cache = AttributionCache::default();
    let mut first = None;
    for i in 0..(MAX_CACHED_FILES + 36) {
        let (mut s, _) = store();
        put(&mut s, &format!("f{i}"), "user:alice", "k", add("n"));
        let room = Room::open("ws_1", format!("f{i}"), s).unwrap();
        snapshot_cached(&room, 5_000, &cache).unwrap().unwrap();
        assert!(cache.len() <= MAX_CACHED_FILES);
        if i == 0 {
            first = Some(room);
        }
    }
    assert_eq!(cache.len(), MAX_CACHED_FILES);
    let again = snapshot_cached(first.as_ref().unwrap(), 5_000, &cache)
        .unwrap()
        .unwrap();
    assert!(again.scanned, "the oldest entry was evicted");
}

#[test]
fn two_workspaces_never_share_an_entry() {
    let cache = AttributionCache::default();
    let (mut a, _) = store();
    put(&mut a, "f", "user:alice", "k", add("n1"));
    let (mut b, _) = store();
    put(&mut b, "f", "user:bob", "k", add("n1"));
    let room_a = Room::open("ws_a", "f", a).unwrap();
    let room_b = Room::open("ws_b", "f", b).unwrap();
    let ra = snapshot_cached(&room_a, 5_000, &cache).unwrap().unwrap();
    let rb = snapshot_cached(&room_b, 5_000, &cache).unwrap().unwrap();
    assert!(rb.scanned, "the other workspace's entry is not a hit");
    assert_eq!(actor_of(&ra, "n1").as_deref(), Some("user:alice"));
    assert_eq!(actor_of(&rb, "n1").as_deref(), Some("user:bob"));
}

/// A source that answers what it is told, to test what the route does with it.
struct Fake(Result<Option<Read>, Failure>);
impl Snapshots for Fake {
    fn snapshot(
        &self,
        _: &Principal,
        _: &str,
        _: &AttributionCache,
    ) -> Result<Option<Read>, Failure> {
        self.0.clone()
    }
}

fn read(scanned: bool) -> Read {
    Read {
        snapshot: FileSnapshot {
            seq: 1,
            nodes: vec![],
            variables: Default::default(),
        },
        scanned,
    }
}

const TOKEN: &str = "tok-user-alice-001";

fn router(fake: Fake, burst: u32) -> Router {
    let auth: Arc<dyn Authenticator> =
        Arc::new(DevTokens::parse(&format!("{TOKEN}:ws_1:user:alice")).unwrap());
    let rate = Rate {
        burst,
        per_second: 1,
    };
    let big = Rate {
        burst: 1000,
        per_second: 1000,
    };
    let config = Config {
        per_client: big,
        per_principal: rate,
        ..Config::default()
    };
    struct Unused;
    impl meldframe_http::Submitter for Unused {
        fn submit(
            &self,
            _: &str,
            _: &str,
            _: &Principal,
            _: Vec<meldframe_host::Op>,
        ) -> Result<Vec<meldframe_host::Accepted>, Failure> {
            Err(Failure::Unavailable("unused".into()))
        }
        fn submit_atomic(
            &self,
            _: &str,
            _: &str,
            _: &Principal,
            _: Vec<meldframe_host::Op>,
            _: Vec<meldframe_http::Guard>,
        ) -> Result<Vec<meldframe_host::Accepted>, Failure> {
            Err(Failure::Unavailable("unused".into()))
        }
    }
    let state = meldframe_http::shared(auth, Arc::new(Unused), config);
    meldframe_http_snapshot::routes(state, Arc::new(fake))
}

async fn status(app: &Router) -> (u16, serde_json::Value) {
    let req = Request::get("/v1/files/f")
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let code = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (code, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tokio::test]
async fn a_call_that_scanned_costs_more_budget_than_one_that_hit_the_cache() {
    let scanning = router(Fake(Ok(Some(read(true)))), 20);
    let mut codes = Vec::new();
    for _ in 0..4 {
        codes.push(status(&scanning).await.0);
    }
    assert_eq!(codes, [200, 200, 200, 429], "scans drain the budget");
    let cached = router(Fake(Ok(Some(read(false)))), 20);
    for _ in 0..20 {
        assert_eq!(status(&cached).await.0, 200, "a cache hit costs one call");
    }
}

#[tokio::test]
async fn a_permanent_store_failure_is_invalid_and_a_transient_one_is_unavailable() {
    let app = router(Fake(Err(Failure::Permanent("x".into()))), 20);
    let (code, v) = status(&app).await;
    assert_eq!((code, v["error"]["code"].as_str()), (400, Some("invalid")));
    let app = router(Fake(Err(Failure::Unavailable("down".into()))), 20);
    let (code, v) = status(&app).await;
    assert_eq!(
        (
            code,
            v["error"]["code"].as_str(),
            v["error"]["retryable"].as_bool()
        ),
        (503, Some("unavailable"), Some(true))
    );
}
