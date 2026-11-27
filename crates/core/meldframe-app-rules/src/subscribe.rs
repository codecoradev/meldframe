//! A refused `subscribe` on the live socket (audit finding T14, issue 324): after a resync marker or a gap the app asks for
//! a subscribe on the socket it holds, and the server may refuse it. What to do about it is decided here, and the shell only
//! reports the refusal: send the subscribe again after the wait the server named (one second when it named none), at most
//! `MAX_SUBSCRIBE_RETRIES` times, then give the connection up (a wait the server names is cut at `MAX_RETRY_AFTER_MS`, so
//! a server cannot park the stream with a huge one); a `bad_request` means the subscribe came before the marker,
//! which is on its way, so wait for it; any other refusal will not get better by waiting. Time arrives as a number.

/// Most times a refused `subscribe` is sent again before the connection counts as lost.
pub const MAX_SUBSCRIBE_RETRIES: u32 = 3;
/// How long to wait before sending a refused `subscribe` again when the server named no time.
pub const RETRY_FALLBACK_MS: u64 = 1_000;
/// The longest wait before a refused `subscribe` is sent again, whatever the server names: a hostile or faulty server must not
/// be able to park the stream for hours (the reader of the frame takes a wait up to 2^53 - 1 ms). The one rule that differs
/// from the TypeScript on purpose: its `setTimeout` overflowed above 2^31 - 1 ms and fired at once.
pub const MAX_RETRY_AFTER_MS: u64 = 30_000;

/// What to do with a refused `subscribe`.
#[derive(Debug, PartialEq, Eq)]
pub enum SubscribeRefusal {
    /// Send it again after `after_ms` (this is retry number `attempt`, from 1).
    Retry { attempt: u32, after_ms: u64 },
    /// Do nothing: the marker it waits for is on its way.
    Wait,
    /// Give the connection up: it goes offline and reconnects from the last confirmed number.
    Lost,
}

/// The retries of a refused `subscribe` that are under way: how many were made, from which number the subscribe asks,
/// and when the next one is due.
#[derive(Debug, Default)]
pub struct Resubscribe {
    tries: u32,
    after_seq: u64,
    due: Option<u64>,
}

impl Resubscribe {
    /// A subscribe that is not a retry goes out from `after_seq`: the count starts again and a retry that waited is cancelled.
    pub fn start(&mut self, after_seq: u64) {
        *self = Resubscribe {
            tries: 0,
            after_seq,
            due: None,
        };
    }

    /// Forget it all: the connection is new, or gone.
    pub fn clear(&mut self) {
        *self = Resubscribe::default();
    }

    /// The server refused the subscribe with `code` (`None`: none, or one that cannot be read) and asked for a wait of
    /// `retry_after_ms`, at `now_ms`. A refusal while a retry waits answers a subscribe that is already being replaced:
    /// it is nothing.
    pub fn on_refused(
        &mut self,
        code: Option<&str>,
        retry_after_ms: Option<u64>,
        now_ms: u64,
    ) -> SubscribeRefusal {
        if self.due.is_some() {
            return SubscribeRefusal::Wait;
        }
        match code {
            Some("rate_limited" | "unavailable") if self.tries >= MAX_SUBSCRIBE_RETRIES => {
                SubscribeRefusal::Lost
            }
            Some("rate_limited" | "unavailable") => {
                let after_ms = retry_after_ms
                    .unwrap_or(RETRY_FALLBACK_MS)
                    .min(MAX_RETRY_AFTER_MS);
                self.tries += 1;
                self.due = Some(now_ms.saturating_add(after_ms));
                SubscribeRefusal::Retry {
                    attempt: self.tries,
                    after_ms,
                }
            }
            Some("bad_request") => SubscribeRefusal::Wait,
            _ => SubscribeRefusal::Lost,
        }
    }

    /// The number a retry that is due asks from, once; `None` while nothing is due.
    pub fn take_due(&mut self, now_ms: u64) -> Option<u64> {
        if self.due.is_some_and(|at| at <= now_ms) {
            self.due = None;
            return Some(self.after_seq);
        }
        None
    }
}
