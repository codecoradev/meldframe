//! The call log in Postgres (E2.4): table `calls` of migration 0005. [`PgCallLog`] keeps the newest entries in memory
//! for [`CallLog::recent`] and writes every entry to the table on the store's own runtime, so a call under the
//! server's lock never waits for the database. A row holds names and codes only, as a [`CallEntry`] does.

use super::{PgStore, db_err};
use meldframe_calls::{CallEntry, CallLog, Decision, MemoryCallLog};
use meldframe_host::StoreError;
use meldframe_module::RiskTier;
use sqlx::Row;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

/// Entries the writer may hold before the log drops new ones (and counts them) rather than block a call.
pub const CALL_QUEUE: usize = 10_000;

/// The call log of a server on Postgres.
pub struct PgCallLog {
    store: PgStore,
    memory: MemoryCallLog,
    tx: mpsc::Sender<CallEntry>,
    /// Entries handed to the writer, entries it finished (written or failed), entries dropped on a full queue.
    sent: Arc<AtomicU64>,
    done: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    /// Entries the writer could not insert (the database refused or was down).
    failed: Arc<AtomicU64>,
}

impl std::fmt::Debug for PgCallLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgCallLog")
            .field("dropped", &self.dropped.load(Ordering::Relaxed))
            .field("failed", &self.failed.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl PgCallLog {
    /// A log on `store`'s database; the newest [`CALL_QUEUE`] entries also stay in memory.
    pub fn new(store: PgStore) -> Self {
        let (tx, mut rx) = mpsc::channel::<CallEntry>(CALL_QUEUE);
        let done = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));
        let (pool, finished, lost) = (store.pool.clone(), done.clone(), failed.clone());
        store.rt.spawn(async move {
            while let Some(e) = rx.recv().await {
                // A row that cannot be written is lost (and counted); the call it describes has already ended.
                let written = sqlx::query(
                    "insert into calls (at_ms, workspace_id, actor, tool, file, risk, decision, outcome) \
                     values ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(i64::try_from(e.at).unwrap_or(i64::MAX))
                .bind(&e.workspace)
                .bind(&e.actor)
                .bind(&e.tool)
                .bind(&e.file)
                .bind(e.risk.map(RiskTier::as_str))
                .bind(e.decision.map(Decision::as_str))
                .bind(&e.outcome)
                .execute(&pool)
                .await;
                if written.is_err() {
                    lost.fetch_add(1, Ordering::Relaxed);
                }
                finished.fetch_add(1, Ordering::SeqCst);
            }
        });
        Self {
            store,
            memory: MemoryCallLog::with_capacity(CALL_QUEUE),
            tx,
            sent: Arc::default(),
            done,
            dropped: Arc::default(),
            failed,
        }
    }

    /// Wait until every entry handed to the writer is finished, at most 10 s (tests, and a clean shutdown).
    pub fn flush(&self) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.done.load(Ordering::SeqCst) < self.sent.load(Ordering::SeqCst)
            && std::time::Instant::now() < end
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Entries the database did not take.
    pub fn failed(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }

    /// Entries dropped because the writer was behind by [`CALL_QUEUE`].
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// At most `limit` of the newest entries of `workspace` in the table, oldest first. Blocks: call it off the
    /// async runtime, as the other `PgStore` methods.
    pub fn of_workspace(
        &self,
        workspace: &str,
        limit: usize,
    ) -> Result<Vec<CallEntry>, StoreError> {
        let rows = self
            .store
            .rt
            .block_on(
                sqlx::query(
                    "select at_ms, workspace_id, actor, tool, file, risk, decision, outcome from calls \
                     where workspace_id = $1 order by id desc limit $2",
                )
                .bind(workspace)
                .bind(i64::try_from(limit).unwrap_or(i64::MAX))
                .fetch_all(&self.store.pool),
            )
            .map_err(db_err)?;
        let mut out: Vec<CallEntry> = rows
            .iter()
            .map(|r| CallEntry {
                at: u64::try_from(r.get::<i64, _>("at_ms")).unwrap_or(0),
                workspace: r.get("workspace_id"),
                actor: r.get("actor"),
                tool: r.get("tool"),
                file: r.get("file"),
                risk: r.get::<Option<String>, _>("risk").as_deref().and_then(tier),
                decision: r
                    .get::<Option<String>, _>("decision")
                    .as_deref()
                    .and_then(decision),
                outcome: r.get("outcome"),
            })
            .collect();
        out.reverse();
        Ok(out)
    }
}

fn tier(s: &str) -> Option<RiskTier> {
    RiskTier::ALL.into_iter().find(|t| t.as_str() == s)
}

fn decision(s: &str) -> Option<Decision> {
    [Decision::Allow, Decision::Ask, Decision::Deny]
        .into_iter()
        .find(|d| d.as_str() == s)
}

/// The writer runs on the store's runtime, which goes when the last clone of the store goes: write what is queued
/// first (at most 10 s), so a server that stops cleanly keeps its last calls.
impl Drop for PgCallLog {
    fn drop(&mut self) {
        self.flush();
    }
}

impl CallLog for PgCallLog {
    fn record(&mut self, entry: CallEntry) {
        self.memory.record(entry.clone());
        match self.tx.try_send(entry) {
            Ok(()) => {
                self.sent.fetch_add(1, Ordering::SeqCst);
            }
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn recent(&self, limit: usize) -> Vec<CallEntry> {
        self.memory.recent(limit)
    }
}
