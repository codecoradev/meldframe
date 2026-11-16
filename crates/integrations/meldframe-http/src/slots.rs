//! Places among the blocking calls. REST and the handshake of a live socket use one set, the live calls
//! after the handshake another, so sockets cannot take every place from REST and a stuck server lock cannot
//! grow the blocked jobs without bound in either.

use meldframe_host::Principal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// The places of one set: a global count and a count per principal.
#[derive(Debug)]
pub struct Slots {
    global: Arc<Semaphore>,
    per_principal: Arc<Mutex<HashMap<String, usize>>>,
    max_per_principal: usize,
}

/// A place; held by the job itself, so it is freed when the job ends, not when the caller stops waiting.
#[derive(Debug)]
pub struct Slot {
    _global: OwnedSemaphorePermit,
    per_principal: Arc<Mutex<HashMap<String, usize>>>,
    key: String,
}

impl Slots {
    pub fn new(global: usize, per_principal: usize) -> Self {
        Slots {
            global: Arc::new(Semaphore::new(global.max(1))),
            per_principal: Arc::default(),
            max_per_principal: per_principal.max(1),
        }
    }

    /// A place for one more call of `who`, or `None` when all are taken.
    pub fn take(&self, who: &Principal) -> Option<Slot> {
        let global = self.global.clone().try_acquire_owned().ok()?;
        let key = format!("{}\u{0}{}", who.workspace, who.actor);
        let mut m = self.per_principal.lock().unwrap_or_else(|p| p.into_inner());
        let n = m.entry(key.clone()).or_insert(0);
        if *n >= self.max_per_principal {
            return None;
        }
        *n += 1;
        Some(Slot {
            _global: global,
            per_principal: self.per_principal.clone(),
            key,
        })
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut m = self.per_principal.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(n) = m.get_mut(&self.key) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                m.remove(&self.key);
            }
        }
    }
}
