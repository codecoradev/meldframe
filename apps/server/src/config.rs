//! The capacities a server starts with: open rooms, how long a room may idle and how many calls the log keeps.

/// How many files the server keeps open at once; opening more fails with `RegistryError::Full`.
pub const MAX_OPEN_ROOMS: usize = 256;

/// A room untouched for this many milliseconds is closed by the next write (see `Server::with_idle_after`).
pub const IDLE_AFTER_MS: u64 = 5 * 60 * 1000;

/// Entries the default call log keeps; the oldest go first.
pub const MAX_CALL_LOG: usize = 10_000;
