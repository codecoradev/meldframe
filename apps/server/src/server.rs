//! The assembled server: the registry of compiled-in modules, the one admission gate and one room per open file.

use crate::approval::{self, APPROVAL_TTL_MS};
use crate::clock::now;
use crate::config::{IDLE_AFTER_MS, MAX_CALL_LOG, MAX_OPEN_ROOMS};
use crate::limits::{
    self, MAX_BATCH_OPS, MAX_OPS_PER_MINUTE, MAX_ROOMS_PER_WORKSPACE, MAX_STORAGE_BYTES,
};
use crate::live::{LIVE_CONFIG, Live, MAX_LIVE_STREAMS};
use crate::modules::modules;
use crate::owners::Owners;
use meldframe_approvals::{ApprovalGate, Approvals};
use meldframe_calls::{CallLog, MemoryCallLog};
use meldframe_host::Room;
use meldframe_module::{Module, Registry, default_policy};
use meldframe_rooms::{MemoryActivityLog, RegistryError};
use meldframe_store_pg::{AnyStore, Backing, PgCallLog, room_key};
use std::collections::HashSet;

pub(crate) type Factory = Box<dyn FnMut(&str) -> AnyStore + Send>;
pub(crate) type Rooms = meldframe_rooms::Rooms<AnyStore, MemoryActivityLog, Factory>;

/// The assembled server: the registry of compiled-in modules, the one admission gate and one room per open
/// file. A write reaches a room only as `Gate` then `Admission` then `Rooms::accept`; no `&mut Room` leaves
/// this type, and an `Admission` cannot be built by hand (`meldframe-rooms` compile-fail tests).
///
/// The store calls of a Postgres backing block on the store's own runtime: call the server from a plain
/// thread or `spawn_blocking`, never from inside an async task.
#[derive(Debug)]
pub struct Server {
    pub(crate) registry: Registry,
    pub(crate) gate: ApprovalGate,
    /// The approvals of calls the gate said Ask for (E2.4), in memory.
    pub(crate) approvals: Approvals,
    pub(crate) rooms: Rooms,
    pub(crate) idle_after: u64,
    pub(crate) live: Live,
    pub(crate) owners: Owners,
    /// Keys of the rooms this server opened and has not closed, so `close_all` can reach them.
    pub(crate) open_keys: HashSet<String>,
    /// One entry for every write call, refused or not (see `meldframe_calls::CallLog`).
    pub(crate) calls: Box<dyn CallLog>,
}

impl Server {
    /// Build the registry from the compiled-in feature modules, with every file in memory.
    ///
    /// # Panics
    ///
    /// When two modules clash on a name: that is a build error in the composition, not a runtime case.
    pub fn assemble() -> Self {
        Self::assemble_in(Vec::new(), Backing::memory())
    }

    /// Like [`Server::assemble`], with more modules registered after the compiled-in ones (tests and
    /// embedders; a clash is a panic as above).
    pub fn assemble_with(extra: Vec<Box<dyn Module>>) -> Self {
        Self::assemble_in(extra, Backing::memory())
    }

    /// Like [`Server::assemble_with`], with the files kept in `backing`. A file is opened, and restored
    /// from its snapshot and log, the first time a write or `open` names it.
    pub fn assemble_in(extra: Vec<Box<dyn Module>>, backing: Backing) -> Self {
        let mut registry = Registry::new();
        for module in modules().into_iter().chain(extra) {
            registry
                .register(module.as_ref())
                .expect("modules do not clash");
        }
        // With Postgres a workspace starts from what the store holds, so its totals survive a restart.
        let pg = match &backing {
            Backing::Postgres(p) => Some(p.clone()),
            Backing::Memory(_) => None,
        };
        let factory: Factory = Box::new(move |_file| backing.store());
        let approvals = Approvals::new(APPROVAL_TTL_MS);
        let mut gate = approvals.gate(|_, tier| default_policy(tier));
        approval::tune(&mut gate, |g| {
            g.batch_limit(MAX_BATCH_OPS)
                .ops_per_minute(MAX_OPS_PER_MINUTE, now)
                .storage_limit(MAX_STORAGE_BYTES)
        });
        // With Postgres the call log is kept there too, so it survives a restart (E2.4).
        let calls: Box<dyn CallLog> = match &pg {
            Some(p) => Box::new(PgCallLog::new(p.clone())),
            None => Box::new(MemoryCallLog::with_capacity(MAX_CALL_LOG)),
        };
        if let Some(pg) = pg {
            let recent = pg.clone();
            approval::tune(&mut gate, |g| {
                g.usage(move |ws| pg.workspace_usage(ws).ok().map(|u| (u.ops, u.bytes)))
                    .recent(move |ws, since| recent.ops_since(ws, since).ok())
            });
        }
        Server {
            registry,
            gate,
            approvals,
            rooms: meldframe_rooms::Rooms::new(MAX_OPEN_ROOMS, factory, MemoryActivityLog::new())
                .with_workspace_limit(MAX_ROOMS_PER_WORKSPACE),
            idle_after: IDLE_AFTER_MS,
            live: meldframe_stream::Streams::new(LIVE_CONFIG, MAX_LIVE_STREAMS),
            owners: Owners::default(),
            open_keys: HashSet::new(),
            calls,
        }
    }

    /// Set the quotas of every workspace: at most `ops_per_minute` ops in a minute and `storage_bytes` bytes of ops,
    /// with `now_ms` as the clock (milliseconds). Defaults are [`MAX_OPS_PER_MINUTE`] and [`MAX_STORAGE_BYTES`].
    /// With Postgres a workspace starts from what the store holds the first time a call names it (its totals, and
    /// the ops of the current minute on `now_ms`, compared with the time the database gave each op); in memory
    /// the counts start from zero.
    pub fn with_quotas(
        mut self,
        ops_per_minute: u64,
        storage_bytes: u64,
        now_ms: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        approval::tune(&mut self.gate, |g| {
            g.ops_per_minute(ops_per_minute, now_ms)
                .storage_limit(storage_bytes)
        });
        self
    }

    /// Count the bytes `extra(workspace)` gives (its assets, ADR-0015) against the storage limit of the workspace,
    /// together with the bytes of its ops.
    pub fn with_extra_storage(
        mut self,
        extra: impl Fn(&str) -> u64 + Send + Sync + 'static,
    ) -> Self {
        approval::tune(&mut self.gate, |g| g.extra_storage(extra));
        self
    }

    /// The bytes of ops a workspace has written and the storage limit it is held to.
    pub fn ops_storage(&mut self, workspace: &str) -> (u64, Option<u64>) {
        (self.gate.storage_of(workspace), self.gate.storage_cap())
    }

    /// Close rooms idle for `ms` milliseconds at the start of each write (default [`IDLE_AFTER_MS`]).
    pub fn with_idle_after(mut self, ms: u64) -> Self {
        self.idle_after = ms;
        self
    }

    /// Let each workspace hold at most `limit` open rooms (default [`MAX_ROOMS_PER_WORKSPACE`]); the
    /// global bound stays [`MAX_OPEN_ROOMS`]. A call over the limit fails with
    /// `RegistryError::WorkspaceFull`.
    pub fn with_workspace_rooms(mut self, limit: usize) -> Self {
        self.rooms = self.rooms.with_workspace_limit(limit);
        self
    }

    /// What the server was built with: modules, tools and their tiers.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Open `file` of `workspace` (idempotent) and return the sequence number of its last op. A file name
    /// belongs to one workspace: another workspace's file of the same name is a different, empty file.
    pub fn open(&mut self, workspace: &str, file: &str) -> Result<u64, RegistryError> {
        limits::valid_names(workspace, file)?;
        let key = room_key(workspace, file);
        let seq = self.open_room(workspace, &key, now())?.seq;
        self.open_keys.insert(key);
        Ok(seq)
    }

    /// Open a room. When a cap refuses it, rooms idle past `idle_after` are closed and the open is tried
    /// once more, so a workspace that only reads or streams gets its idle slots back without a write.
    pub(crate) fn open_room(
        &mut self,
        workspace: &str,
        key: &str,
        at: u64,
    ) -> Result<meldframe_rooms::Opened, RegistryError> {
        match self.rooms.open(workspace, key, at) {
            Err(RegistryError::Full { .. } | RegistryError::WorkspaceFull { .. }) => {
                self.close_idle(at);
                self.rooms.open(workspace, key, at)
            }
            other => other,
        }
    }

    pub(crate) fn close_idle(&mut self, at: u64) {
        for key in self.rooms.close_idle(at, self.idle_after) {
            self.open_keys.remove(&key);
        }
    }

    /// Close `file`; every accepted op is already in the store, so a later open restores it.
    pub fn close(&mut self, workspace: &str, file: &str) -> bool {
        let key = room_key(workspace, file);
        self.open_keys.remove(&key);
        self.rooms.close(&key)
    }

    /// Close every open room; a later open restores each from its store. For a caller that cannot trust the
    /// in-memory rooms any more, such as after a panic left one half changed.
    pub fn close_all(&mut self) {
        for key in std::mem::take(&mut self.open_keys) {
            self.rooms.close(&key);
        }
    }

    /// Read an open room; `None` when the file is not open. There is no way to write through this.
    pub fn with_room<R>(
        &self,
        workspace: &str,
        file: &str,
        f: impl FnOnce(&Room<AnyStore>) -> R,
    ) -> Option<R> {
        self.rooms.with_room(&room_key(workspace, file), f)
    }
}
