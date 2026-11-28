//! Postgres test setup: a fresh schema per call, dropped when the store goes out of scope.
#![allow(dead_code)]

use meldframe_host::{Appended, NewOp, Snapshot, Store, StoreError, StoredOp};
use meldframe_server::PgStore;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// The database to test against. Without `DATABASE_URL` the Postgres tests skip (CI's `migrations` job
/// has a Postgres service; `just dev-db` starts one locally). With `MELDFRAME_REQUIRE_PG` set (CI does)
/// a missing `DATABASE_URL` is a failure, so a misconfigured job cannot pass by skipping everything.
pub fn database_url() -> Option<String> {
    let url = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty());
    if url.is_none() && std::env::var_os("MELDFRAME_REQUIRE_PG").is_some() {
        panic!("MELDFRAME_REQUIRE_PG is set but DATABASE_URL is not");
    }
    url
}

/// A schema name that no other test, process or earlier run uses.
pub fn unique_schema() -> String {
    format!(
        "t_{}_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    )
}

/// A store on a schema of its own that is dropped with it. Clones made with `store()` share the schema.
pub struct Scratch {
    store: PgStore,
    url: String,
    pub schema: String,
}

impl Scratch {
    pub fn new(url: &str) -> Self {
        let schema = unique_schema();
        let store = PgStore::connect_in_schema(url, &schema).expect("connect and migrate");
        Scratch {
            store,
            url: url.to_string(),
            schema,
        }
    }

    /// A second connection to the same schema (a restart, or another process).
    pub fn reconnect(&self) -> PgStore {
        PgStore::connect_in_schema(&self.url, &self.schema).expect("reconnect")
    }

    pub fn store(&self) -> PgStore {
        self.store.clone()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = PgStore::drop_schema(&self.url, &self.schema);
    }
}

/// A store on an empty, migrated schema of its own, or `None` when there is no database.
pub fn fresh() -> Option<Scratch> {
    Some(Scratch::new(&database_url()?))
}

impl Store for Scratch {
    fn append(&mut self, room: &str, op: NewOp) -> Result<Appended, StoreError> {
        self.store.append(room, op)
    }
    fn find(&self, room: &str, actor: &str, key: &str) -> Result<Option<u64>, StoreError> {
        self.store.find(room, actor, key)
    }
    fn latest_seq(&self, room: &str) -> Result<u64, StoreError> {
        self.store.latest_seq(room)
    }
    fn ops_after(&self, room: &str, seq: u64, limit: usize) -> Result<Vec<StoredOp>, StoreError> {
        self.store.ops_after(room, seq, limit)
    }
    fn save_snapshot(&mut self, room: &str, snapshot: Snapshot) -> Result<(), StoreError> {
        self.store.save_snapshot(room, snapshot)
    }
    fn latest_snapshot(&self, room: &str) -> Result<Option<Snapshot>, StoreError> {
        self.store.latest_snapshot(room)
    }
}
