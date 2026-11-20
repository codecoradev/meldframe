//! What the app layer keeps beside the reducer: the memo of `inEffect`, and heal rule (d) under way. The reducer
//! leaves rules (c) and (d) to the app layer; this is where they are written, with no I/O and no clock.

use crate::{Activity, FETCH_LIMIT, Fetch, InEffect, MAX_ROWS, RawHeader, in_effect};
use meldframe_revert::ConfirmedLog;

/// Rule (d) asks for at most this many pages after the first: 5 pages of 100, the row cap.
const HEAL_PAGES: u8 = 4;

impl Fetch {
    /// The newest page: heal rules (c) and (d).
    pub const NEWEST: Fetch = Fetch {
        before_n: None,
        after_n: None,
        limit: FETCH_LIMIT,
    };

    /// The page below `n`.
    pub const fn before(n: u32) -> Fetch {
        Fetch {
            before_n: Some(n),
            ..Fetch::NEWEST
        }
    }
}

/// Rule (d) under way: the lowest and the highest `n` held when it began, the pages it may still ask for, and the `n`
/// the page it waits for must lie below (`None` while the newest page is awaited).
#[derive(Debug, Clone, Copy)]
struct Heal {
    floor: u32,
    top: u32,
    left: u8,
    below: Option<u32>,
}

/// The key of a memoised `inEffect`: the confirmed `seq`, where the record starts, how many ops it holds, and the batch.
type Key = (u64, u64, usize, String, String);

/// The answer of the list route as the reducer takes it: a page of headers, the server's count and whether older ones
/// remain, and whether this person may set review states.
#[derive(Debug, Clone)]
pub struct Page {
    pub headers: Vec<RawHeader>,
    pub unreviewed: u32,
    pub has_more: bool,
    pub can_review: bool,
}

/// The reducer with the memo of the selected row's `inEffect` and the state of heal rule (d).
#[derive(Debug)]
pub struct Feed {
    pub red: Activity,
    memo: Option<(Key, InEffect)>,
    heal: Option<Heal>,
    /// The batch the person picked (actor and key prefix), until its row goes.
    pub(crate) selected: Option<(String, String)>,
    /// What the server last said of this person: may they set review states.
    pub(crate) can_review: bool,
}

impl Feed {
    /// A reducer for `me` over a log that starts after `base_seq`.
    pub fn new(me: &str, base_seq: u64) -> Self {
        let mut red = Activity::new(me);
        red.reset_log(base_seq);
        Feed {
            red,
            memo: None,
            heal: None,
            selected: None,
            can_review: false,
        }
    }

    /// The log started again (a reload): everything goes as [`Activity::reset_all`] says, and so do the memo and rule (d).
    pub fn reset(&mut self, base_seq: u64) {
        self.red.reset_all(base_seq);
        (self.memo, self.heal) = (None, None);
    }

    /// The clock moved. While the connection is down only the clock runs: no try is spent and nothing is asked.
    pub fn tick(&mut self, now_ms: u64, live: bool) -> Option<Fetch> {
        if live {
            return self.red.tick(now_ms);
        }
        self.red.clock(now_ms);
        None
    }

    /// Rule (d), after a subscribe or a resync: read the newest page, then down to the lowest `n` held while pages are
    /// full ([`Feed::page`]), at most 5 pages.
    pub fn connected(&mut self, now_ms: u64) -> Option<Fetch> {
        self.red.spend_window(now_ms);
        self.heal = Some(Heal {
            floor: self.red.lowest_n().unwrap_or(0),
            top: self.red.highest_n().unwrap_or(0),
            left: HEAL_PAGES,
            below: None,
        });
        Some(Fetch::NEWEST)
    }

    /// Whether rule (d) is under way, waiting for a page.
    pub fn healing(&self) -> bool {
        self.heal.is_some()
    }

    /// A `batch` frame arrived at `now_ms`: the clock moves first (so rule (a) can tell a second gap from the first),
    /// then the header is taken in. The fetch rule (a) asks for, if any.
    pub fn header(
        &mut self,
        now_ms: u64,
        header: impl Into<RawHeader>,
        unreviewed: u32,
    ) -> Option<Fetch> {
        self.red.clock(now_ms);
        self.red.observe_header(header, unreviewed)
    }

    /// A `touch` frame arrived at `now_ms`.
    pub fn touch(&mut self, now_ms: u64, actor: &str, ids: &[String]) {
        self.red.observe_touch(actor, ids, now_ms);
    }

    /// The page below the oldest `n` held, if the numbers reach lower and the rows are not at the cap (a page past it
    /// would be dropped at once).
    pub fn older(&self) -> Option<Fetch> {
        let room = self.red.rows().len() < MAX_ROWS;
        self.red
            .lowest_n()
            .filter(|n| *n > 1 && room)
            .map(Fetch::before)
    }

    /// Pick a batch (Show): its node ids, or `None` when it is not a row.
    pub fn select(&mut self, actor: &str, prefix: &str) -> Option<Vec<String>> {
        let there = self
            .red
            .rows()
            .iter()
            .any(|r| (r.actor.as_str(), r.key_prefix.as_str()) == (actor, prefix));
        if there {
            self.selected = Some((actor.to_owned(), prefix.to_owned()));
        }
        there.then(|| self.red.show_ids(actor, prefix))
    }

    /// The number the server knows a batch by, or the code that refuses a review of it: `unknown_batch` (not a row) or
    /// `no_batch_number` (a row with no header yet).
    pub fn number(&self, actor: &str, prefix: &str) -> Result<u32, &'static str> {
        let rows = self.red.rows();
        let row = rows
            .iter()
            .find(|r| (r.actor.as_str(), r.key_prefix.as_str()) == (actor, prefix));
        row.ok_or("unknown_batch")?.n.ok_or("no_batch_number")
    }

    /// A page arrived: fold it in. Rule (d) goes on down with it only when it is the answer it waits for (the newest
    /// page, which reaches down to what was held or is full, or a page below the last `n` it asked below), has more
    /// below it and has not reached the floor; any other answer (a page another rule asked for, such as the small
    /// answer of rule (b) above the highest `n` held, or one of a connection that is gone) leaves the chain as it is.
    /// An empty answer to a request below ends it.
    pub fn page(&mut self, p: Page) -> Option<Fetch> {
        let Page {
            headers,
            unreviewed,
            has_more,
            can_review,
        } = p;
        self.can_review = can_review;
        let len = headers.len();
        let span = headers
            .iter()
            .map(|h| h.n)
            .min()
            .zip(headers.iter().map(|h| h.n).max());
        self.red.observe_page(headers, unreviewed);
        let h = self.heal?;
        let Some((low, high)) = span else {
            // An empty answer to a request below ends the chain; while the newest page is awaited it may be the answer
            // of another rule (a header not written yet), so the chain waits on.
            return h.below.and_then(|_| self.failed());
        };
        // The newest page reaches down to what is held (or is full, or nothing was held); a page above the highest `n`
        // held that is neither (the small answer of rule (b)) is not it. Later: a page below the last `n` asked below.
        let fits = match h.below {
            None => h.top == 0 || low <= h.top || len >= FETCH_LIMIT as usize,
            Some(b) => high < b,
        };
        if !fits {
            return None;
        }
        if has_more && low > h.floor && h.left > 0 {
            self.heal = Some(Heal {
                left: h.left - 1,
                below: Some(low),
                ..h
            });
            return Some(Fetch::before(low));
        }
        self.failed()
    }

    /// A fetch failed, or rule (d) is done: it stops, and nothing asks again by itself: the state waits for rule (a),
    /// (b) or (c). Always `None`, the fetch there is not.
    pub fn failed(&mut self) -> Option<Fetch> {
        self.heal = None;
        None
    }

    /// Whether a revert of the batch would still change anything, read from the confirmed log and kept while the
    /// log (its newest `seq`, where it starts, how many ops it holds) and the batch stay the same.
    pub fn in_effect(
        &mut self,
        log: &ConfirmedLog,
        seq: u64,
        actor: &str,
        prefix: &str,
    ) -> InEffect {
        let key = (
            seq,
            log.base_seq(),
            log.ops().len(),
            actor.to_owned(),
            prefix.to_owned(),
        );
        if let Some((k, v)) = &self.memo
            && *k == key
        {
            return *v;
        }
        let v = in_effect(log.base(), log.ops(), actor, prefix);
        self.memo = Some((key, v));
        v
    }
}
