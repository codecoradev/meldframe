//! Which rows go when a new one is made, and when it is refused (ADR-0023, bounds and retention).
//!
//! The rule differs from the batch headers of ADR-0021, which refuse at their caps: a checkpoint the server takes on
//! its own is never refused, because the newest run start is the one a person needs, and one caller must not be able
//! to push another's rows out. So each kind has its share of the file, rows roll only within their own share, and a
//! `manual` row is refused at its share and never rolled out.

use crate::{Checkpoint, MAX_MANUAL_PER_ACTOR, MAX_RUN_START_PER_ACTOR, NewCheckpoint, Share};

/// The plan for one new row: the number it takes, the counter after it, and the rows of the file that go with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rolled {
    pub n: u32,
    pub next_n: u32,
    /// The numbers of the rows to delete, in the same transaction and under the same lock as the insert.
    pub delete: Vec<u32>,
}

/// Why no row is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A `manual` row over its share, or over the cap of its actor (`checkpoint_full`).
    Full { limit: usize, what: &'static str },
    /// The counter is not above every row held, or has run out: a number would be reused (`unavailable`).
    Counter,
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::Full { .. } => "checkpoint_full",
            Refusal::Counter => "unavailable",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Full { limit, what } => {
                write!(f, "at most {limit} {what}: the limit is reached")
            }
            Refusal::Counter => write!(f, "the checkpoint counter of the file is not usable"),
        }
    }
}

impl std::error::Error for Refusal {}

/// The plan for `new` in a file that holds `held` (every row of the file, in any order) with the counter at
/// `next_n`. A `manual` row is refused at the share of the file and then at the cap of its actor; the automatic
/// kinds never are: a run start rolls its agent's oldest rows past [`MAX_RUN_START_PER_ACTOR`] and then the oldest of
/// the share, and a before row the oldest of its share. Oldest is the lowest `n`.
pub fn roll(held: &[Checkpoint], next_n: u32, new: &NewCheckpoint) -> Result<Rolled, Refusal> {
    if next_n == 0 || next_n == u32::MAX || held.iter().any(|c| c.n >= next_n) {
        return Err(Refusal::Counter);
    }
    let share = new.kind().share();
    let mut same: Vec<&Checkpoint> = held.iter().filter(|c| c.kind.share() == share).collect();
    same.sort_by_key(|c| c.n);
    let mut delete = Vec::new();
    match share {
        Share::Manual => {
            if same.len() >= share.limit() {
                return Err(full(share.limit(), share.what()));
            }
            let mine = same.iter().filter(|c| c.by.as_deref() == new.by()).count();
            if mine >= MAX_MANUAL_PER_ACTOR {
                return Err(full(
                    MAX_MANUAL_PER_ACTOR,
                    "manual checkpoints of one actor",
                ));
            }
        }
        Share::Run => {
            let own: Vec<u32> = same
                .iter()
                .filter(|c| c.about.as_deref() == new.about())
                .map(|c| c.n)
                .collect();
            let over = (own.len() + 1).saturating_sub(MAX_RUN_START_PER_ACTOR);
            delete.extend(own.into_iter().take(over));
            roll_share(&same, share, &mut delete);
        }
        Share::Before => roll_share(&same, share, &mut delete),
    }
    Ok(Rolled {
        n: next_n,
        next_n: next_n + 1,
        delete,
    })
}

/// Add the oldest rows of the share that still have to go for one more to fit, those already chosen apart.
fn roll_share(same: &[&Checkpoint], share: Share, delete: &mut Vec<u32>) {
    let left = same.iter().filter(|c| !delete.contains(&c.n));
    let over = (left.count() + 1).saturating_sub(share.limit());
    let more: Vec<u32> = same
        .iter()
        .map(|c| c.n)
        .filter(|n| !delete.contains(n))
        .take(over)
        .collect();
    delete.extend(more);
}

fn full(limit: usize, what: &'static str) -> Refusal {
    Refusal::Full { limit, what }
}
