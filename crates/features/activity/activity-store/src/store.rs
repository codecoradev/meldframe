//! The in-memory store of batch headers.

use crate::{
    ActivityError, BatchStore, Caps, NoteOutcome, Page, Query, Recorded, Reviewed, cap_reached,
    check_actor, check_batch, check_query, merge, new_header,
};
use meldframe_activity::{Header, NewBatch, Review};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// The headers of one file and its counters, as the counter row of Postgres keeps them.
#[derive(Debug, Default)]
struct File {
    next_n: u32,
    next_rev: u64,
    /// In number order: the header `n` is at `n - 1`, since a number is never reused and headers are never removed.
    rows: Vec<Header>,
    /// The index of the header of an (actor, key prefix).
    by_key: HashMap<(String, String), usize>,
    per_actor: HashMap<String, usize>,
    open: u32,
}

impl File {
    fn take_rev(&mut self) -> u64 {
        self.next_rev += 1;
        self.next_rev
    }
}

/// Headers in memory, for tests and a server without a database. A restart loses them, as it loses the file's ops.
#[derive(Debug, Default)]
pub struct MemoryBatches {
    caps: Caps,
    files: Mutex<HashMap<(String, String), File>>,
}

impl MemoryBatches {
    /// A store with caps other than the default.
    pub fn with_caps(caps: Caps) -> Self {
        MemoryBatches {
            caps,
            files: Mutex::default(),
        }
    }
}

fn key(workspace: &str, file: &str) -> (String, String) {
    (workspace.to_owned(), file.to_owned())
}

impl BatchStore for MemoryBatches {
    fn record(
        &self,
        workspace: &str,
        file: &str,
        batch: &NewBatch,
    ) -> Result<Recorded, ActivityError> {
        check_batch(batch)?;
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        let in_workspace: usize = files
            .iter()
            .filter(|((w, _), _)| w == workspace)
            .map(|(_, f)| f.rows.len())
            .sum();
        let id = (batch.actor.clone(), batch.key_prefix.clone());
        let found = files
            .get(&key(workspace, file))
            .and_then(|f| f.by_key.get(&id).copied());
        if found.is_none() {
            // The caps come first, so a refused call leaves no file behind.
            let (mine, rows) = files.get(&key(workspace, file)).map_or((0, 0), |f| {
                (
                    f.per_actor.get(&batch.actor).copied().unwrap_or(0),
                    f.rows.len(),
                )
            });
            if let Some(full) = cap_reached(&self.caps, mine, rows, in_workspace) {
                return Err(full);
            }
        }
        let f = files.entry(key(workspace, file)).or_default();
        if let Some(i) = found {
            let mut h = f.rows[i].clone();
            let (changed, note) = merge(&mut h, batch);
            if changed {
                h.rev = f.take_rev();
                f.rows[i] = h.clone();
            }
            return Ok(Recorded {
                header: h,
                created: false,
                changed,
                note,
                unreviewed: f.open,
            });
        }
        f.next_n += 1;
        let (n, rev) = (f.next_n, f.take_rev());
        let h = new_header(n, rev, batch);
        f.rows.push(h.clone());
        f.by_key.insert(id, f.rows.len() - 1);
        *f.per_actor.entry(batch.actor.clone()).or_default() += 1;
        f.open += 1;
        let note = match &batch.note {
            Some(_) => NoteOutcome::Kept,
            None => NoteOutcome::None,
        };
        Ok(Recorded {
            header: h,
            created: true,
            changed: true,
            note,
            unreviewed: f.open,
        })
    }

    fn list(&self, workspace: &str, file: &str, query: &Query) -> Result<Page, ActivityError> {
        check_query(query)?;
        let files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(f) = files.get(&key(workspace, file)) else {
            return Ok(Page {
                headers: Vec::new(),
                has_more: false,
                unreviewed: 0,
            });
        };
        let mut hit = f.rows.iter().rev().filter(|h| {
            query.before.is_none_or(|b| h.n < b) && query.after.is_none_or(|a| h.n > a)
        });
        let headers: Vec<Header> = hit.by_ref().take(query.limit).cloned().collect();
        Ok(Page {
            headers,
            has_more: hit.next().is_some(),
            unreviewed: f.open,
        })
    }

    fn set_review(
        &self,
        workspace: &str,
        file: &str,
        n: u32,
        state: Review,
        by: &str,
        at_ms: u64,
    ) -> Result<Reviewed, ActivityError> {
        check_actor(by)?;
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        let f = files
            .get_mut(&key(workspace, file))
            .ok_or(ActivityError::NotFound(n))?;
        let i = (n as usize)
            .checked_sub(1)
            .filter(|i| *i < f.rows.len())
            .ok_or(ActivityError::NotFound(n))?;
        if f.rows[i].review == state {
            return Ok(Reviewed {
                header: f.rows[i].clone(),
                changed: false,
                unreviewed: f.open,
            });
        }
        let was_open = f.rows[i].review.is_open();
        let rev = f.take_rev();
        let h = &mut f.rows[i];
        (h.review, h.rev) = (state, rev);
        (h.reviewed_by, h.reviewed_at_ms) = (Some(by.to_owned()), Some(at_ms));
        let h = h.clone();
        match (was_open, state.is_open()) {
            (true, false) => f.open -= 1,
            (false, true) => f.open += 1,
            _ => {}
        }
        Ok(Reviewed {
            header: h,
            changed: true,
            unreviewed: f.open,
        })
    }
}
