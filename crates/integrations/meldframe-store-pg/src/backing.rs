//! Where the files are kept: in memory, or in Postgres through [`PgStore`]. The server picks one with [`Backing`].

use crate::PgStore;
use meldframe_host::{Appended, MemoryStore, NewOp, Snapshot, Store, StoreError, StoredOp};
use std::sync::{Arc, Mutex};

/// The store of one room. The registry hands every room its own `AnyStore`; clones of one backing share
/// the same durable state, which is how a closed file comes back with its content.
#[derive(Debug)]
pub enum AnyStore {
    Memory(Arc<Mutex<MemoryStore>>),
    Pg(PgStore),
}

/// Which durable state the server's rooms use.
#[derive(Debug, Clone)]
pub enum Backing {
    /// All files in memory for the life of the process (the default; a restart loses them).
    Memory(Arc<Mutex<MemoryStore>>),
    /// All files in Postgres. Connect once and pass the store: each room gets a clone of it.
    Postgres(PgStore),
}

impl Backing {
    pub fn memory() -> Self {
        Backing::Memory(Arc::default())
    }

    /// Postgres when `DATABASE_URL` is set and not empty (applying the migrations), memory otherwise.
    pub fn from_env() -> Result<Self, StoreError> {
        match std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()) {
            Some(url) => Ok(Backing::Postgres(PgStore::connect(&url)?)),
            None => Ok(Backing::memory()),
        }
    }

    /// The critique lists of the files (E5.4): in Postgres with the files, else in memory (a restart loses them).
    pub fn reviews(&self) -> Arc<dyn design_review::ReviewStore> {
        match self {
            Backing::Memory(_) => Arc::new(design_review::MemoryReviews::default()),
            Backing::Postgres(p) => Arc::new(p.clone()),
        }
    }

    /// The batch headers of the files (E5.1): in Postgres with the files, else in memory (a restart loses them).
    pub fn batches(&self) -> Arc<dyn activity_store::BatchStore> {
        match self {
            Backing::Memory(_) => Arc::new(activity_store::MemoryBatches::default()),
            Backing::Postgres(p) => Arc::new(p.clone()),
        }
    }

    /// The store of one room: a clone of the backing, so rooms of one backing share its durable state.
    pub fn store(&self) -> AnyStore {
        match self {
            Backing::Memory(m) => AnyStore::Memory(m.clone()),
            Backing::Postgres(p) => AnyStore::Pg(p.clone()),
        }
    }
}

fn poisoned<T>(_: T) -> StoreError {
    StoreError::Unavailable("the in-memory store lock is poisoned".into())
}

impl Store for AnyStore {
    fn append(&mut self, room: &str, op: NewOp) -> Result<Appended, StoreError> {
        match self {
            AnyStore::Memory(m) => m.lock().map_err(poisoned)?.append(room, op),
            AnyStore::Pg(p) => p.append(room, op),
        }
    }

    fn find(&self, room: &str, actor: &str, key: &str) -> Result<Option<u64>, StoreError> {
        match self {
            AnyStore::Memory(m) => m.lock().map_err(poisoned)?.find(room, actor, key),
            AnyStore::Pg(p) => p.find(room, actor, key),
        }
    }

    fn latest_seq(&self, room: &str) -> Result<u64, StoreError> {
        match self {
            AnyStore::Memory(m) => m.lock().map_err(poisoned)?.latest_seq(room),
            AnyStore::Pg(p) => p.latest_seq(room),
        }
    }

    fn ops_after(&self, room: &str, seq: u64, limit: usize) -> Result<Vec<StoredOp>, StoreError> {
        match self {
            AnyStore::Memory(m) => m.lock().map_err(poisoned)?.ops_after(room, seq, limit),
            AnyStore::Pg(p) => p.ops_after(room, seq, limit),
        }
    }

    fn save_snapshot(&mut self, room: &str, snapshot: Snapshot) -> Result<(), StoreError> {
        match self {
            AnyStore::Memory(m) => m.lock().map_err(poisoned)?.save_snapshot(room, snapshot),
            AnyStore::Pg(p) => p.save_snapshot(room, snapshot),
        }
    }

    fn latest_snapshot(&self, room: &str) -> Result<Option<Snapshot>, StoreError> {
        match self {
            AnyStore::Memory(m) => m.lock().map_err(poisoned)?.latest_snapshot(room),
            AnyStore::Pg(p) => p.latest_snapshot(room),
        }
    }
}
