//! The in-memory store of checkpoints.

use crate::{CheckpointError, CheckpointStore, Held, Made, Plan, plan};
use meldframe_activity::store_key;
use meldframe_checkpoints::{Checkpoint, NewCheckpoint};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// The rows of one file with the key each was made with, and its counter, as the counter row of Postgres keeps it.
#[derive(Debug)]
struct File {
    next_n: u32,
    /// In number order: a roll removes rows and a number is never given twice.
    rows: Vec<(Checkpoint, Option<String>)>,
}

impl File {
    fn new() -> Self {
        File {
            next_n: 1,
            rows: Vec::new(),
        }
    }
}

/// Rows in memory, for tests and a server without a database. A restart loses them, as it loses the file's ops.
#[derive(Debug, Default)]
pub struct MemoryCheckpoints {
    files: Mutex<HashMap<(String, String), File>>,
}

impl MemoryCheckpoints {
    fn apply(
        f: &mut File,
        key_of_request: Option<&str>,
        new: &NewCheckpoint,
        at_ms: u64,
    ) -> Result<Made, CheckpointError> {
        let held: Vec<Held<'_>> = f
            .rows
            .iter()
            .map(|(row, k)| Held {
                row,
                key: k.as_deref(),
            })
            .collect();
        match plan(&held, f.next_n, key_of_request, new)? {
            Plan::Existing(n) => {
                let row = f
                    .rows
                    .iter()
                    .find(|(c, _)| c.n == n)
                    .map(|(c, _)| c.clone());
                let checkpoint =
                    row.ok_or_else(|| CheckpointError::Unavailable("row lost".into()))?;
                Ok(Made {
                    checkpoint,
                    created: false,
                    rolled: Vec::new(),
                })
            }
            Plan::Insert(r) => {
                f.rows.retain(|(c, _)| !r.delete.contains(&c.n));
                let checkpoint = Checkpoint {
                    n: r.n,
                    seq: new.seq(),
                    kind: new.kind(),
                    label: new.label().cloned(),
                    about: new.about().map(str::to_owned),
                    by: new.by().map(str::to_owned),
                    at_ms,
                };
                f.rows
                    .push((checkpoint.clone(), key_of_request.map(str::to_owned)));
                f.next_n = r.next_n;
                Ok(Made {
                    checkpoint,
                    created: true,
                    rolled: r.delete,
                })
            }
        }
    }
}

impl CheckpointStore for MemoryCheckpoints {
    fn create(
        &self,
        workspace: &str,
        file: &str,
        key_of_request: Option<&str>,
        new: &NewCheckpoint,
        at_ms: u64,
    ) -> Result<Made, CheckpointError> {
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        // A file that is not there yet is made only when a row is: a refused call leaves nothing behind.
        let k = store_key(workspace, file);
        let mut f = files.remove(&k).unwrap_or_else(File::new);
        let made = Self::apply(&mut f, key_of_request, new, at_ms);
        if f.next_n > 1 {
            files.insert(k, f);
        }
        made
    }

    fn list(&self, workspace: &str, file: &str) -> Result<Vec<Checkpoint>, CheckpointError> {
        let files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        let rows = files.get(&store_key(workspace, file)).map(|f| &f.rows);
        Ok(rows.map_or_else(Vec::new, |r| {
            r.iter().rev().map(|(c, _)| c.clone()).collect()
        }))
    }
}
