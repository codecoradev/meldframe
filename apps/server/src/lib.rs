//! Server composition root. It assembles the registry from the compiled-in feature modules and builds the
//! room registry (one room per file) over an in-memory or Postgres backing. HTTP, MCP, REST and WebSocket
//! transports are the `meldframe-http` adapter over [`SharedServer`]; the binary only assembles and holds no
//! business logic. [`PgStore`] is the
//! Postgres adapter of the store seam; [`Backing`] picks it.

mod http;
mod pg;

pub use http::SharedServer;
pub use pg::PgStore;

use meldframe_host::{Accepted, Gate, Op, Principal, Refusal, Room, RoomError, Store};
use meldframe_module::{Module, Policy, Registry, RiskTier};
use meldframe_rooms::{MemoryActivityLog, RegistryError, Rooms, TreeFacts};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

mod store;

pub use store::{AnyStore, Backing};

/// How many files the server keeps open at once; opening more fails with `RegistryError::Full`.
pub const MAX_OPEN_ROOMS: usize = 256;

/// A room untouched for this many milliseconds is closed by the next write (see `Server::with_idle_after`).
pub const IDLE_AFTER_MS: u64 = 5 * 60 * 1000;

type Factory = Box<dyn FnMut(&str) -> AnyStore + Send>;

/// Why `Server::submit_tool` wrote nothing, or stopped part way.
#[derive(Debug, Clone, PartialEq)]
pub enum SubmitError {
    /// The tool or the gate refused the batch; nothing was written.
    Refused(Refusal),
    /// The file would not open, or the room refused or failed an op. Ops of the same batch that came before
    /// stay written (each op is its own durable write); sending the batch again with the same keys returns
    /// those as `written: false` and writes only the rest.
    Rooms(RegistryError),
}

impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubmitError::Refused(r) => write!(f, "{r}"),
            SubmitError::Rooms(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SubmitError {}

impl SubmitError {
    /// The refusal behind this error, whether the tool, the gate or the room refused: one shape (and one
    /// stable `Refusal::code`) for a transport. `None` for a failure that is not a refusal (the store is
    /// down, the registry is full, the log does not replay).
    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            SubmitError::Refused(r)
            | SubmitError::Rooms(RegistryError::Room(RoomError::Refused(r))) => Some(r),
            SubmitError::Rooms(_) => None,
        }
    }
}

impl From<Refusal> for SubmitError {
    fn from(r: Refusal) -> Self {
        SubmitError::Refused(r)
    }
}

impl From<RegistryError> for SubmitError {
    fn from(e: RegistryError) -> Self {
        SubmitError::Rooms(e)
    }
}

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
    rooms: Rooms<AnyStore, MemoryActivityLog, Factory>,
    idle_after: u64,
}

/// The key of a file in the registry and the store: the workspace's length, the workspace, then the file.
/// The length prefix makes it injective (workspace `a/b` with file `c` is not workspace `a` with file `b/c`),
/// so a workspace never reaches another workspace's file by choosing its name.
fn room_key(workspace: &str, file: &str) -> String {
    format!("{}:{workspace}{file}", workspace.len())
}

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
            gate: Gate::new(),
            rooms: Rooms::new(MAX_OPEN_ROOMS, factory, MemoryActivityLog::new()),
            idle_after: IDLE_AFTER_MS,
        }
    }

    /// Close rooms idle for `ms` milliseconds at the start of each write (default [`IDLE_AFTER_MS`]).
    pub fn with_idle_after(mut self, ms: u64) -> Self {
        self.idle_after = ms;
        self
    }

    /// What the server was built with: modules, tools and their tiers.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Open `file` of `workspace` (idempotent) and return the sequence number of its last op. A file name
    /// belongs to one workspace: another workspace's file of the same name is a different, empty file.
    pub fn open(&mut self, workspace: &str, file: &str) -> Result<u64, RegistryError> {
        let key = room_key(workspace, file);
        Ok(self.rooms.open(workspace, &key, now())?.seq)
    }

    /// Close `file`; every accepted op is already in the store, so a later open restores it.
    pub fn close(&mut self, workspace: &str, file: &str) -> bool {
        self.rooms.close(&room_key(workspace, file))
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
    /// stops at a failing op leaves the later ops charged and unwritten, and a retry is charged again. The
    /// server sets no quota today; fixing it needs a host change.
    pub fn submit_tool(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, SubmitError> {
        let at = now();
        self.rooms.close_idle(at, self.idle_after);
        known_auto_tool(&self.registry, tool, &ops)?;
        let key = room_key(&who.workspace, file);
        let opened = self.rooms.open(&who.workspace, &key, at)?;
        let done = self.write(tool, &key, who, ops, at);
        if done.is_err() && !opened.already_open {
            self.rooms.close(&key);
        }
        done
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
        let report = rooms
            .with_room(key, |room| {
                let facts = TreeFacts::new(room.nodes());
                check_tool(registry, tool, &fresh, &facts)?;
                gate.submit_with(who, fresh, &facts)
            })
            .expect("the room was just opened")?;
        let mut written = report.admitted.into_iter();
        for slot in done.iter_mut().filter(|d| d.is_none()) {
            let admission = written.next().expect("one admission per op");
            *slot = Some(rooms.accept(key, admission, at)?.accepted);
        }
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
    ]
}
