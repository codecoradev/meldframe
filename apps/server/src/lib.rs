//! Server composition root. It assembles the registry from the compiled-in feature modules and builds the
//! room registry (one room per file) over an in-memory or Postgres backing. HTTP, MCP, REST and WebSocket
//! transports are the `meldframe-http` adapter over [`SharedServer`]; the binary only assembles and holds no
//! business logic. [`PgStore`] is the
//! Postgres adapter of the store seam; [`Backing`] picks it.

mod error;
mod http;
mod limits;
mod live;
mod owners;

pub use error::SubmitError;
pub use http::{SharedServer, failure_of};
pub use limits::{
    MAX_BATCH_OPS, MAX_NAME_BYTES, MAX_OPS_PER_MINUTE, MAX_ROOMS_PER_WORKSPACE, MAX_STORAGE_BYTES,
};
pub use live::{
    Frame, LIVE_CONFIG, MAX_LIVE_STREAMS, StreamId, SubscribeError, Subscription, TOUCH_WINDOW_MS,
};
pub use meldframe_store_pg::{PERMANENT_STORE_ERROR, PgStore};
pub use meldframe_stream::{Config, StreamError};

use meldframe_calls::{CallEntry, CallLog, MemoryCallLog};
use meldframe_host::{Accepted, Gate, Op, Principal, Refusal, Room, Store, StoredOp};
use meldframe_module::{Module, Policy, Registry, RiskTier};
use meldframe_rooms::{MemoryActivityLog, RegistryError, TreeFacts};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

mod store;

pub use store::{AnyStore, Backing};

/// How many files the server keeps open at once; opening more fails with `RegistryError::Full`.
pub const MAX_OPEN_ROOMS: usize = 256;

/// A room untouched for this many milliseconds is closed by the next write (see `Server::with_idle_after`).
pub const IDLE_AFTER_MS: u64 = 5 * 60 * 1000;

type Factory = Box<dyn FnMut(&str) -> AnyStore + Send>;
pub(crate) type Rooms = meldframe_rooms::Rooms<AnyStore, MemoryActivityLog, Factory>;

/// The assembled server: the registry of compiled-in modules, the one admission gate and one room per open
/// file. A write reaches a room only as `Gate` then `Admission` then `Rooms::accept`; no `&mut Room` leaves
/// this type, and an `Admission` cannot be built by hand (`meldframe-rooms` compile-fail tests).
///
/// The store calls of a Postgres backing block on the store's own runtime: call the server from a plain
/// thread or `spawn_blocking`, never from inside an async task.
#[derive(Debug)]
pub struct Server {
    registry: Registry,
    gate: Gate,
    rooms: Rooms,
    idle_after: u64,
    live: live::Live,
    owners: owners::Owners,
    /// Keys of the rooms this server opened and has not closed, so `close_all` can reach them.
    open_keys: HashSet<String>,
    /// One entry for every write call, refused or not (see `meldframe_calls::CallLog`).
    calls: Box<dyn CallLog>,
}

/// The key of a file in the registry and the store: the workspace's length, the workspace, then the file.
/// The length prefix makes it injective (workspace `a/b` with file `c` is not workspace `a` with file `b/c`),
/// so a workspace never reaches another workspace's file by choosing its name.
fn room_key(workspace: &str, file: &str) -> String {
    format!("{}:{workspace}{file}", workspace.len())
}

/// Entries the default call log keeps; the oldest go first.
pub const MAX_CALL_LOG: usize = 10_000;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
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
        let factory: Factory = Box::new(move |_file| backing.store());
        Server {
            registry,
            gate: Gate::new()
                .batch_limit(MAX_BATCH_OPS)
                .ops_per_minute(MAX_OPS_PER_MINUTE, now)
                .storage_limit(MAX_STORAGE_BYTES),
            rooms: meldframe_rooms::Rooms::new(MAX_OPEN_ROOMS, factory, MemoryActivityLog::new())
                .with_workspace_limit(MAX_ROOMS_PER_WORKSPACE),
            idle_after: IDLE_AFTER_MS,
            live: meldframe_stream::Streams::new(LIVE_CONFIG, MAX_LIVE_STREAMS),
            owners: owners::Owners::default(),
            open_keys: HashSet::new(),
            calls: Box::new(MemoryCallLog::with_capacity(MAX_CALL_LOG)),
        }
    }

    /// Set the quotas of every workspace: at most `ops_per_minute` ops in a minute and `storage_bytes` bytes of ops,
    /// with `now_ms` as the clock (milliseconds). Defaults are [`MAX_OPS_PER_MINUTE`] and [`MAX_STORAGE_BYTES`].
    /// The counts start from zero: a gate built here does not know what was written before.
    pub fn with_quotas(
        mut self,
        ops_per_minute: u64,
        storage_bytes: u64,
        now_ms: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        self.gate = std::mem::take(&mut self.gate)
            .ops_per_minute(ops_per_minute, now_ms)
            .storage_limit(storage_bytes);
        self
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

    fn close_idle(&mut self, at: u64) {
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

    /// Submit ops on behalf of an agent tool to `file` of the principal's workspace. The registry's tier
    /// for the tool gives its policy and is an upper bound on the tier of every op: a Read tool cannot
    /// change data and a Change tool cannot delete. A tool the registry does not know is `Deny` (fail
    /// closed), and `Ask` counts as forbidden until approval exists, as in the gate. The gate then applies
    /// its own tier policy to each op and is the only source of a permit. The tree of the file's room, as it
    /// is now, tells the gate which deletes are small (ADR-0010): a small delete is a Change, so a Change
    /// tool may submit it. Each admitted op is then accepted by the room of `file`, in order, and recorded
    /// in the activity log. Returns the accepted ops (`written: false` for a retried key).
    ///
    /// An unknown or denied tool is refused before any room is opened, and a room this call opened is
    /// closed again when the call fails, so refused calls cannot fill the registry; idle rooms are closed
    /// first. An op whose key was already applied with the same command is a duplicate: it is answered with
    /// its first sequence number and skips the tier decision (the tree it was decided on may have changed,
    /// as after a delete) and the gate, since nothing is written.
    ///
    /// Known limit: the gate counts quota when it admits a batch, before the room accepts. A batch that
    /// stops at a failing op leaves the later ops charged and unwritten, and a retry is charged again (rate,
    /// storage and total alike); refunding needs a host change. The quotas are checked after the file is open.
    pub fn submit_tool(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, SubmitError> {
        let done = self.submit_tool_unlogged(tool, file, who, ops);
        self.record_call(tool, file, who, done.as_ref().err());
        done
    }

    fn submit_tool_unlogged(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, SubmitError> {
        limits::valid_names(&who.workspace, file)?;
        let at = now();
        for key in self.rooms.close_idle(at, self.idle_after) {
            self.open_keys.remove(&key);
        }
        known_auto_tool(&self.registry, tool, &ops)?;
        limits::precheck(self.registry.agent_tool(tool).map(|t| t.tier), &ops)?;
        let key = room_key(&who.workspace, file);
        let opened = self.open_room(&who.workspace, &key, at)?;
        self.open_keys.insert(key.clone());
        let done = self.write(tool, &key, who, ops, at);
        if done.is_err() && !opened.already_open {
            self.rooms.close(&key);
            self.open_keys.remove(&key);
        }
        done
    }

    /// Keep one entry for a write call that ended in `failure` (`None`: it was written). `submit_tool` does this by
    /// itself; a caller that refuses a call before it reaches `submit_tool` (an atomic batch that failed its
    /// check) calls it for that call.
    pub fn record_call(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        failure: Option<&SubmitError>,
    ) {
        self.record_call_as(
            tool,
            file,
            who,
            failure.map_or(Ok(()), |e| Err(e.refusal())),
        );
    }

    /// The same for a caller that has only the refusal: `Ok(())` for a call that went through, `Err(Some(refusal))`
    /// for one a policy or the room refused, `Err(None)` for a failure that is not a refusal.
    pub fn record_call_as(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        done: Result<(), Option<&Refusal>>,
    ) {
        let tier = self.registry.agent_tool(tool).map(|t| t.tier);
        self.calls
            .record(CallEntry::new(now(), who, tool, file, tier, done));
    }

    /// At most `limit` of the newest calls, oldest first.
    pub fn calls(&self, limit: usize) -> Vec<CallEntry> {
        self.calls.recent(limit)
    }

    /// Replace the call log (an embedder that keeps it elsewhere); the default keeps the newest [`MAX_CALL_LOG`]
    /// in memory.
    pub fn with_call_log(mut self, log: Box<dyn CallLog>) -> Self {
        self.calls = log;
        self
    }

    /// The checks of `submit_tool` that need no room: names, batch size, a known tool with an `Auto` policy,
    /// and a Read tool with an op that is a write whatever the tree. A caller that opens the room itself
    /// first (an atomic batch) runs this before opening it.
    pub fn precheck(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: &[Op],
    ) -> Result<(), SubmitError> {
        limits::valid_names(&who.workspace, file)?;
        known_auto_tool(&self.registry, tool, ops)?;
        limits::precheck(self.registry.agent_tool(tool).map(|t| t.tier), ops)?;
        Ok(())
    }

    fn write(
        &mut self,
        tool: &str,
        key: &str,
        who: &Principal,
        ops: Vec<Op>,
        at: u64,
    ) -> Result<Vec<Accepted>, SubmitError> {
        let Server {
            registry,
            gate,
            rooms,
            live,
            ..
        } = self;
        let mut done: Vec<Option<Accepted>> = ops
            .iter()
            .map(|op| {
                rooms
                    .with_room(key, |room| duplicate(room, &who.actor, op))
                    .flatten()
            })
            .collect();
        let fresh: Vec<Op> = ops
            .into_iter()
            .zip(&done)
            .filter(|(_, d)| d.is_none())
            .map(|(op, _)| op)
            .collect();
        if fresh.is_empty() && !done.is_empty() {
            return Ok(done.into_iter().flatten().collect());
        }
        let mut sent = fresh.clone().into_iter();
        let report = rooms
            .with_room(key, |room| {
                let facts = TreeFacts::new(room.nodes());
                check_tool(registry, tool, &fresh, &facts)?;
                gate.submit_with(who, fresh, &facts)
            })
            .expect("the room was just opened")?;
        let mut written = report.admitted.into_iter();
        // The nodes the batch wrote and its newest number, for one touch hint.
        let (mut touched, mut newest) = (Vec::new(), 0);
        for slot in done.iter_mut().filter(|d| d.is_none()) {
            let admission = written.next().expect("one admission per op");
            let op = sent.next().expect("one op per admission");
            let accepted = rooms.accept(key, admission, at)?.accepted;
            if accepted.written {
                touched.extend(meldframe_batch::touches(&op.command));
                newest = accepted.seq;
                let stored = StoredOp {
                    seq: accepted.seq,
                    actor: who.actor.clone(),
                    key: op.key,
                    command: op.command,
                };
                // A gap cannot happen (one writer, one numbering); if it did, the hub resyncs its streams
                // itself, so the write is never failed here.
                let _ = live.publish(key, accepted.seq, stored);
            }
            *slot = Some(accepted);
        }
        live.touch(key, &who.actor, touched, newest, at);
        Ok(done.into_iter().flatten().collect())
    }
}

/// `Some` when `op`'s key is already in the room's log for `actor` with the same command.
fn duplicate(room: &Room<AnyStore>, actor: &str, op: &Op) -> Option<Accepted> {
    let store = room.store();
    let seq = store.find(room.id(), actor, &op.key).ok()??;
    let first = store.ops_after(room.id(), seq - 1, 1).ok()?.pop()?;
    let same =
        serde_json::to_value(&first.command).ok()? == serde_json::to_value(&op.command).ok()?;
    (first.seq == seq && same).then_some(Accepted {
        seq,
        written: false,
    })
}

/// The tool is known and its policy is `Auto`; returns its tier. A tool that fails is refused with the highest
/// tier among `ops` (the tool's own tier when the tool is known).
fn tool_tier(
    registry: &Registry,
    tool: &str,
    highest: Option<RiskTier>,
) -> Result<RiskTier, Refusal> {
    let Some(tool_tier) = registry.agent_tool(tool).map(|t| t.tier) else {
        return Err(forbidden(highest.unwrap_or(RiskTier::Read), Policy::Deny));
    };
    match registry.policy_for(tool) {
        Some(Policy::Auto) => Ok(tool_tier),
        Some(policy) => Err(forbidden(tool_tier, policy)),
        None => Err(forbidden(tool_tier, Policy::Deny)),
    }
}

fn highest_tier<'a>(ops: &'a [Op], tier: impl Fn(&'a Op) -> RiskTier) -> Option<RiskTier> {
    ops.iter().map(tier).max_by_key(|t| rank(*t))
}

/// Before any room is opened: known tool, `Auto` policy. Without the tree a delete cannot be told small,
/// so the cap on op tiers waits for [`check_tool`].
fn known_auto_tool(registry: &Registry, tool: &str, ops: &[Op]) -> Result<(), Refusal> {
    tool_tier(registry, tool, highest_tier(ops, Op::tier)).map(|_| ())
}

/// The whole tool check: as above, and no op above the tool's tier, with the tree to tell small deletes.
fn check_tool(
    registry: &Registry,
    tool: &str,
    ops: &[Op],
    facts: &TreeFacts<'_>,
) -> Result<(), Refusal> {
    let highest = highest_tier(ops, |op| op.tier_with(facts));
    let tier = tool_tier(registry, tool, highest)?;
    match highest.filter(|t| rank(*t) > rank(tier)) {
        Some(op_tier) => Err(forbidden(op_tier, Policy::Deny)),
        None => Ok(()),
    }
}

fn rank(tier: RiskTier) -> u8 {
    match tier {
        RiskTier::Read => 0,
        RiskTier::Change => 1,
        RiskTier::Delete => 2,
    }
}

fn forbidden(tier: RiskTier, policy: Policy) -> Refusal {
    Refusal::Forbidden { tier, policy }
}

/// The feature modules compiled into this build: one entry per enabled cargo feature.
fn modules() -> Vec<Box<dyn Module>> {
    vec![
        #[cfg(feature = "example")]
        Box::new(example::ExampleModule),
        #[cfg(feature = "mcp")]
        Box::new(meldframe_http::mcp::McpModule),
        #[cfg(feature = "capture")]
        Box::new(capture::CaptureModule),
    ]
}
