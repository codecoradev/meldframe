//! The room registry: one room per file, opened on demand, bounded, and written only through the registry.
//!
//! The registry owns its rooms, so a write needs `&mut Rooms`: Rust allows one writer at a time, and each room
//! still has one total order of sequence numbers. No lock, thread or channel is needed here. A transport that
//! shares the registry between tasks wraps it in its own single owner (a mutex or an actor) at the app layer.
//! The registry hands out read access (`with_room`) and `accept`, never `&mut Room`: a write that skipped
//! `accept` would skip the activity log.

use crate::{ActivityLog, Logged, accept_logged};
use meldframe_host::{Admission, Room, RoomError, Store};
use std::collections::HashMap;
use std::fmt;

/// What `Rooms::open` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opened {
    /// The sequence number of the room's last accepted op, 0 for a new file.
    pub seq: u64,
    /// True when the file was already open: the same room, untouched, and no store was asked for.
    pub already_open: bool,
}

/// Why a registry call failed. These are failures of the registry, not refusals of an op (`Refusal`, with its
/// wire codes, stays in `RoomError::Refused`). A transport maps them to its own errors.
#[derive(Debug, Clone, PartialEq)]
pub enum RegistryError {
    /// The registry holds its limit of open rooms; close one first. Opening a file that is already open
    /// never hits this.
    Full { limit: usize },
    /// The workspace holds its own limit of open rooms (see `Rooms::with_workspace_limit`); close one of
    /// its files first. Other workspaces are not affected. Opening a file that is already open never hits
    /// this.
    WorkspaceFull { limit: usize },
    /// A workspace or file name the server does not accept (empty, too long, a control character). Raised
    /// by the server before the registry is asked, so a store never sees the name.
    InvalidName { reason: String },
    /// The file is not open.
    NotOpen { file: String },
    /// The file is open for another workspace.
    WorkspaceMismatch { file: String },
    /// The room did not open (the store failed or the log does not replay) or refused or failed the op.
    Room(RoomError),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::Full { limit } => write!(f, "{limit} rooms are open; close one first"),
            RegistryError::WorkspaceFull { limit } => {
                write!(f, "the workspace has {limit} rooms open; close one first")
            }
            RegistryError::InvalidName { reason } => write!(f, "invalid name: {reason}"),
            RegistryError::NotOpen { file } => write!(f, "file {file} is not open"),
            RegistryError::WorkspaceMismatch { file } => {
                write!(f, "file {file} is open for another workspace")
            }
            RegistryError::Room(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RegistryError {}

impl From<RoomError> for RegistryError {
    fn from(e: RoomError) -> Self {
        RegistryError::Room(e)
    }
}

/// An open room and the caller-clock value of its last touch.
struct Slot<S: Store> {
    room: Room<S>,
    touched: u64,
}

impl<S: Store> Slot<S> {
    /// The last touch never moves back: a clock value from the past is ignored.
    fn touch(&mut self, at: u64) {
        self.touched = self.touched.max(at);
    }
}

/// At most `max_open` rooms, one per file, with one activity log for all of them.
///
/// `make_store` gives a handle on the file's durable store. It is called once per actual open (not for a file
/// that is already open) and must hand back the same durable state each time, as a database adapter does:
/// that is how a closed file is restored when it is opened again.
pub struct Rooms<S: Store, L: ActivityLog, F: FnMut(&str) -> S> {
    max_open: usize,
    per_workspace: usize,
    make_store: F,
    log: L,
    open: HashMap<String, Slot<S>>,
    activity_failures: u64,
}

impl<S: Store, L: ActivityLog, F: FnMut(&str) -> S> fmt::Debug for Rooms<S, L, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rooms")
            .field("max_open", &self.max_open)
            .field("open", &self.open.len())
            .field("activity_failures", &self.activity_failures)
            .finish_non_exhaustive()
    }
}

impl<S: Store, L: ActivityLog, F: FnMut(&str) -> S> Rooms<S, L, F> {
    pub fn new(max_open: usize, make_store: F, log: L) -> Self {
        Rooms {
            max_open,
            per_workspace: usize::MAX,
            make_store,
            log,
            open: HashMap::new(),
            activity_failures: 0,
        }
    }

    /// At most `limit` open rooms for any one workspace (default: no limit beyond `max_open`), so one
    /// workspace cannot fill the registry for the others.
    pub fn with_workspace_limit(mut self, limit: usize) -> Self {
        self.per_workspace = limit;
        self
    }

    /// Open the room of `file` in `workspace`, restoring it from its store (latest snapshot, then the log).
    /// Idempotent: a file that is already open is left as it is and reported with `already_open`.
    ///
    /// `at` is the caller's clock value; `open` touches the room (see `close_idle`), also when the file was
    /// already open. A refused open (full, wrong workspace, failed store) touches nothing.
    pub fn open(&mut self, workspace: &str, file: &str, at: u64) -> Result<Opened, RegistryError> {
        if let Some(slot) = self.open.get_mut(file) {
            if slot.room.workspace() != workspace {
                return Err(RegistryError::WorkspaceMismatch { file: file.into() });
            }
            slot.touch(at);
            return Ok(Opened {
                seq: slot.room.seq(),
                already_open: true,
            });
        }
        let held = self
            .open
            .values()
            .filter(|s| s.room.workspace() == workspace)
            .count();
        if held >= self.per_workspace {
            return Err(RegistryError::WorkspaceFull {
                limit: self.per_workspace,
            });
        }
        if self.open.len() >= self.max_open {
            return Err(RegistryError::Full {
                limit: self.max_open,
            });
        }
        let room = Room::open(workspace, file, (self.make_store)(file))?;
        let seq = room.seq();
        self.open
            .insert(file.to_string(), Slot { room, touched: at });
        Ok(Opened {
            seq,
            already_open: false,
        })
    }

    /// Close the room of `file` and free its slot. Every accepted op is already in the store, so nothing is
    /// lost. False when the file was not open.
    pub fn close(&mut self, file: &str) -> bool {
        self.open.remove(file).is_some()
    }

    /// Close every room that has been idle for at least `idle_for` clock units at `now`, and return the
    /// closed file ids sorted, so the answer does not depend on hash order. A room is idle when
    /// `now - last_touch >= idle_for`; a `now` before the last touch (a clock that went backwards) never
    /// closes a room, and nothing underflows. `idle_for` of 0 closes every room whose last touch is not
    /// after `now`; `u64::MAX` closes none unless `now` is itself `u64::MAX` and the touch was 0.
    ///
    /// What touches a room: `open` (also of an open file) and an accepted write through `accept` that was
    /// written (a retried key and a refused op do not). `with_room` is a read and does not touch. The clock
    /// is the caller's, the same unit for `open`, `accept` and here; this crate reads no wall clock.
    /// Closing is `close`: the slot is freed and a later `open` restores the room from its store.
    pub fn close_idle(&mut self, now: u64, idle_for: u64) -> Vec<String> {
        let mut idle: Vec<String> = self
            .open
            .iter()
            .filter(|(_, s)| now.checked_sub(s.touched).is_some_and(|d| d >= idle_for))
            .map(|(file, _)| file.clone())
            .collect();
        idle.sort();
        for file in &idle {
            self.open.remove(file);
        }
        idle
    }

    /// Read an open room; `None` when the file is not open.
    pub fn with_room<R>(&self, file: &str, f: impl FnOnce(&Room<S>) -> R) -> Option<R> {
        self.open.get(file).map(|slot| f(&slot.room))
    }

    /// Accept an admitted op in the room of `file` and record it in the activity log (see `accept_logged`).
    pub fn accept(
        &mut self,
        file: &str,
        admission: Admission,
        at: u64,
    ) -> Result<Logged, RegistryError> {
        let slot = self
            .open
            .get_mut(file)
            .ok_or_else(|| RegistryError::NotOpen { file: file.into() })?;
        let logged = accept_logged(&mut slot.room, &mut self.log, admission, at)?;
        if logged.accepted.written {
            slot.touch(at);
        }
        if logged.log_error.is_some() {
            self.activity_failures += 1;
        }
        Ok(logged)
    }

    pub fn is_open(&self, file: &str) -> bool {
        self.open.contains_key(file)
    }

    /// How many rooms are open.
    pub fn len(&self) -> usize {
        self.open.len()
    }

    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// The activity log of every room.
    pub fn activity(&self) -> &L {
        &self.log
    }

    /// How many activity entries could not be written; the ops are in their stores regardless.
    pub fn activity_failures(&self) -> u64 {
        self.activity_failures
    }
}
