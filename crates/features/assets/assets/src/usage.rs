//! What the assets of each workspace weigh, for the storage quota (ADR-0015: assets count with `quota_storage`).
//!
//! The count is kept in memory and starts, for a workspace, from what the store holds the first time the
//! workspace is named, so a restarted server loses nothing; a store that cannot say is asked again at the next
//! call (an upload is then refused as unavailable, not as over the limit). An upload takes its bytes before it
//! is written, in a [`Reservation`] that gives them back when it is dropped, a panic included, unless the write
//! created the file ([`crate::Stored::created`]): so a failed write, and an image that another upload wrote first,
//! cost nothing, and the same image sent twice at once is charged once. While both are in flight each holds its
//! bytes, so a refusal near the limit can be a little early. It can also be a little late: the ops of a
//! workspace are read before the reservation, so an op batch and an upload at the same moment can pass the limit
//! together by one batch and one image.
//!
//! A count that is unknown is not the same for the two callers. An upload is refused as unavailable (the image is
//! the thing at risk). The gate counts the assets as zero, so that a disk that cannot be read never stops the
//! ops of a workspace; the next walk waits [`RETRY_AFTER`].

use super::BlobStore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a store that failed to say what a workspace holds is left alone before it is asked again, so a broken
/// disk is not walked at every call.
const RETRY_AFTER: Duration = Duration::from_secs(5);

/// Why a reservation was not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reserve {
    /// The assets, the ops and the new bytes do not fit in this limit.
    Full(u64),
    /// The store could not say what the workspace holds: try again later.
    Unknown,
}

/// The bytes of assets of each workspace.
#[derive(Clone)]
pub struct Usage {
    store: Arc<dyn BlobStore>,
    held: Arc<Mutex<HashMap<String, u64>>>,
    /// Workspaces whose scan failed, and when.
    failed: Arc<Mutex<HashMap<String, Instant>>>,
}

impl std::fmt::Debug for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Usage").finish_non_exhaustive()
    }
}

/// Bytes taken for an upload that is being written. Dropped, they go back; [`Reservation::keep`] makes them stay.
#[must_use = "a reservation that is dropped gives its bytes back"]
pub struct Reservation {
    usage: Usage,
    workspace: String,
    bytes: u64,
    kept: bool,
}

impl std::fmt::Debug for Reservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reservation")
            .field("workspace", &self.workspace)
            .field("bytes", &self.bytes)
            .finish()
    }
}

impl Reservation {
    /// The write created the file: the bytes stay counted.
    pub fn keep(mut self) {
        self.kept = true;
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.kept {
            return;
        }
        if let Some(n) = self.usage.lock().get_mut(&self.workspace) {
            *n = n.saturating_sub(self.bytes);
        }
    }
}

impl Usage {
    pub fn new(store: Arc<dyn BlobStore>) -> Self {
        Usage {
            store,
            held: Arc::default(),
            failed: Arc::default(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, u64>> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// What the store holds for a workspace not counted yet. The scan runs without the lock, so a slow disk does
    /// not hold up the calls of other workspaces.
    fn seed(&self, workspace: &str) -> Option<u64> {
        if let Some(n) = self.lock().get(workspace) {
            return Some(*n);
        }
        {
            let failed = self
                .failed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if failed
                .get(workspace)
                .is_some_and(|at| at.elapsed() < RETRY_AFTER)
            {
                return None;
            }
        }
        let scanned = match self.store.used(workspace) {
            Ok(n) => n,
            Err(_) => {
                self.failed
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(workspace.to_owned(), Instant::now());
                return None;
            }
        };
        Some(*self.lock().entry(workspace.to_owned()).or_insert(scanned))
    }

    /// The bytes of assets `workspace` holds, or `None` when the store cannot say yet.
    pub fn used(&self, workspace: &str) -> Option<u64> {
        self.seed(workspace)
    }

    /// Take `bytes` for `workspace` if `others` (what it holds besides assets), its assets and `bytes` fit in
    /// `limit`. Nothing is taken on an error.
    pub fn reserve(
        &self,
        workspace: &str,
        bytes: u64,
        others: u64,
        limit: u64,
    ) -> Result<Reservation, Reserve> {
        self.seed(workspace).ok_or(Reserve::Unknown)?;
        let mut held = self.lock();
        let now = held.get(workspace).copied().unwrap_or(0);
        if others.saturating_add(now).saturating_add(bytes) > limit {
            return Err(Reserve::Full(limit));
        }
        held.insert(workspace.to_owned(), now + bytes);
        Ok(Reservation {
            usage: self.clone(),
            workspace: workspace.to_owned(),
            bytes,
            kept: false,
        })
    }
}
