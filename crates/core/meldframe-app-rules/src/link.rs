//! The timing of the live connection: what the app remembers beyond the public state (failed attempts, when the shell
//! last heard from the server, since when the connection is live, whether a subscribe for a gap is out) and the
//! backoff. Time arrives as a number (`now_ms`, the shell's clock); nothing here reads a clock.

/// A live connection that has been silent this long is dropped. The server sends a heartbeat when it has
/// said nothing for 15 seconds, so three are missed by then.
pub const SILENT_MS: u64 = 45_000;
/// A first connection or a reconnect attempt that has not reached `Connected` after this long has failed.
pub const ATTEMPT_MS: u64 = 10_000;
/// A connection that stayed live this long is stable: the backoff starts again from the first step.
pub const STABLE_MS: u64 = 10_000;
const BACKOFF_FIRST_MS: u64 = 1_000;
const BACKOFF_MAX_MS: u64 = 30_000;

/// What the app remembers about the connection beyond the public state.
#[derive(Debug, Default)]
pub struct Link {
    /// Failed attempts since the connection was last stable.
    failures: u32,
    /// When the shell last heard from the server, or started the attempt that is under way.
    heard_ms: u64,
    /// When the connection became live, until it drops.
    live_since: Option<u64>,
    /// A subscribe for a gap is out; no second one until a frame is applied or the connection is new.
    resub: bool,
}

/// How long to wait before attempt number `step + 1`: 1 s, doubling, up to 30 s.
pub fn backoff(step: u32) -> u64 {
    let doubled = BACKOFF_FIRST_MS.saturating_mul(1u64 << step.saturating_sub(1).min(20));
    doubled.min(BACKOFF_MAX_MS)
}

impl Link {
    /// An attempt starts (the first one, or a reconnect).
    pub fn attempt(&mut self, now_ms: u64) {
        self.heard_ms = now_ms;
        self.resub = false;
    }

    /// The transport is subscribed.
    pub fn live(&mut self, now_ms: u64) {
        self.heard_ms = now_ms;
        self.live_since = Some(now_ms);
        self.resub = false;
    }

    /// The connection dropped or an attempt failed: how long to wait before the next one.
    pub fn failed(&mut self) -> u64 {
        self.failures = self.failures.saturating_add(1);
        self.live_since = None;
        backoff(self.failures)
    }

    /// The file was loaded again: the connection is no longer live.
    pub fn reloaded(&mut self) {
        self.live_since = None;
    }

    /// Failed attempts since the connection was last stable.
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// How long the shell has heard nothing.
    pub fn silent_for(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.heard_ms)
    }

    /// Something arrived while live.
    pub fn heard(&mut self, now_ms: u64) {
        self.heard_ms = now_ms;
        self.stable(now_ms);
    }

    /// A connection that stayed live long enough starts the backoff again.
    pub fn stable(&mut self, now_ms: u64) {
        if self
            .live_since
            .is_some_and(|since| now_ms.saturating_sub(since) >= STABLE_MS)
        {
            self.failures = 0;
        }
    }

    /// A subscribe for a gap goes out.
    pub fn resubscribing(&mut self) {
        self.resub = true;
    }

    /// Whether a subscribe for a gap is out.
    pub fn is_resubscribing(&self) -> bool {
        self.resub
    }

    /// A frame was applied: the subscribe for a gap did its work.
    pub fn gap_closed(&mut self) {
        self.resub = false;
    }
}
