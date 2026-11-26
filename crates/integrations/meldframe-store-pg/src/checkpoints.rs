//! The checkpoints in Postgres (E5.3, ADR-0023): tables `checkpoints` and `checkpoint_files` of migration 0008. A
//! create is one transaction that takes `pg_advisory_xact_lock` on the workspace's checkpoints first, with a key prefix of
//! its own (`checkpoints:`; the batch headers use `batches:`, the design review lists `design_review:`), so numbers
//! are given one after the other and the shares hold when calls run at once; the lock goes with the transaction.
//! The rules are `checkpoint_store::plan`, the same function the memory adapter runs, over the rows of the file
//! (at most 100) read under the lock: this file only keeps them and holds the lock. The counter is read from the
//! counter row of the file, never from `max(n)`, so a number a roll deleted is never given again, and the counts of
//! each share are recounted from the rows in the same transaction, so they cannot drift.

use super::{PgStore, room_key};
use crate::db::{db_i64, from_db_seq};
use checkpoint_store::{CheckpointError, CheckpointStore, Held, Made, Plan, plan};
use meldframe_checkpoints::{Checkpoint, Kind, Label, NewCheckpoint};
use sqlx::{PgConnection, Row, postgres::PgRow};

/// A database error as the seam's: a foreign key violation (a row for a file that has no ops yet) can never succeed
/// as it is, so it is `Invalid`; anything else is `Unavailable`, which a caller may retry. Only `Display` of the error
/// is ever kept.
fn failed(e: sqlx::Error) -> CheckpointError {
    if e.as_database_error().and_then(|d| d.code()).as_deref() == Some("23503") {
        return CheckpointError::Invalid(
            "nothing to checkpoint yet: this file has no ops".to_owned(),
        );
    }
    CheckpointError::Unavailable(format!("{e}"))
}

const COLUMNS: &str = "n, seq, kind, label, about, by, key, at_ms";

/// A stored row and the key it was made with; `None` for a row that the rules would not have written (a kind outside
/// the four), which a hand edit could make and which is left out rather than shown.
fn row(r: &PgRow) -> Option<(Checkpoint, Option<String>)> {
    let kind = Kind::parse(r.get::<&str, _>("kind"))?;
    let checkpoint = Checkpoint {
        n: u32::try_from(r.get::<i32, _>("n")).ok()?,
        seq: from_db_seq(r.get("seq")),
        kind,
        // A stored label was checked by the server; the same rule is applied again, so a row edited by hand cannot
        // bring hidden text into a list.
        label: r
            .get::<Option<String>, _>("label")
            .and_then(|t| Label::parse(&t).ok().flatten()),
        about: r.get("about"),
        by: r.get("by"),
        at_ms: from_db_seq(r.get("at_ms")),
    };
    Some((checkpoint, r.get("key")))
}

/// The lock of a workspace's checkpoints, held until the transaction ends.
async fn lock(tx: &mut PgConnection, workspace: &str) -> Result<(), sqlx::Error> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("checkpoints:{}:{workspace}", workspace.len()))
        .execute(tx)
        .await
        .map(|_| ())
}

async fn read_rows(
    tx: &mut PgConnection,
    room: &str,
) -> Result<Vec<(Checkpoint, Option<String>)>, CheckpointError> {
    let sql = format!("select {COLUMNS} from checkpoints where file_id = $1 order by n");
    let rows = sqlx::query(&sql)
        .bind(room)
        .fetch_all(tx)
        .await
        .map_err(failed)?;
    Ok(rows.iter().filter_map(row).collect())
}

impl PgStore {
    /// Run `f` in a transaction on the runtime of the store, committing when it returns `Ok`. Blocks, like the other
    /// `PgStore` methods: call it off the async runtime.
    fn checkpoint_tx<T>(
        &self,
        f: impl AsyncFnOnce(&mut PgConnection) -> Result<T, CheckpointError>,
    ) -> Result<T, CheckpointError> {
        self.rt.block_on(async {
            let mut tx = self.pool.begin().await.map_err(failed)?;
            let out = f(&mut *tx).await?;
            tx.commit().await.map_err(failed)?;
            Ok(out)
        })
    }
}

impl CheckpointStore for PgStore {
    fn create(
        &self,
        workspace: &str,
        file: &str,
        key: Option<&str>,
        new: &NewCheckpoint,
        at_ms: u64,
    ) -> Result<Made, CheckpointError> {
        let room = room_key(workspace, file);
        self.checkpoint_tx(async |tx| {
            lock(tx, workspace).await.map_err(failed)?;
            // The counter row of the file; the foreign key refuses a file that has no ops.
            sqlx::query(
                "insert into checkpoint_files (file_id, workspace_id) values ($1, $2) on conflict (file_id) do nothing",
            )
            .bind(&room)
            .bind(workspace)
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
            let next: i32 = sqlx::query_scalar("select next_n from checkpoint_files where file_id = $1")
                .bind(&room)
                .fetch_one(&mut *tx)
                .await
                .map_err(failed)?;
            let next = u32::try_from(next).unwrap_or(u32::MAX);
            let rows = read_rows(tx, &room).await?;
            let held: Vec<Held<'_>> = rows
                .iter()
                .map(|(c, k)| Held { row: c, key: k.as_deref() })
                .collect();
            match plan(&held, next, key, new)? {
                Plan::Existing(n) => {
                    let found = rows.iter().find(|(c, _)| c.n == n).map(|(c, _)| c.clone());
                    let checkpoint = found
                        .ok_or_else(|| CheckpointError::Unavailable("the row is gone".into()))?;
                    Ok(Made { checkpoint, created: false, rolled: Vec::new() })
                }
                Plan::Insert(r) => {
                    let gone: Vec<i32> = r.delete.iter().filter_map(|n| i32::try_from(*n).ok()).collect();
                    sqlx::query("delete from checkpoints where file_id = $1 and n = any($2)")
                        .bind(&room)
                        .bind(&gone)
                        .execute(&mut *tx)
                        .await
                        .map_err(failed)?;
                    sqlx::query(
                        "insert into checkpoints (file_id, workspace_id, n, seq, kind, label, about, by, key, at_ms) \
                         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                    )
                    .bind(&room)
                    .bind(workspace)
                    .bind(i32::try_from(r.n).unwrap_or(i32::MAX))
                    .bind(db_i64(new.seq()))
                    .bind(new.kind().as_str())
                    .bind(new.label().map(Label::as_str))
                    .bind(new.about())
                    .bind(new.by())
                    .bind(key)
                    .bind(db_i64(at_ms))
                    .execute(&mut *tx)
                    .await
                    .map_err(failed)?;
                    sqlx::query(
                        "update checkpoint_files set next_n = $2, \
                         manual_rows = (select count(*) from checkpoints where file_id = $1 and kind = 'manual'), \
                         before_rows = (select count(*) from checkpoints where file_id = $1 \
                                        and kind in ('beforeRevert', 'beforeRestore')), \
                         run_rows = (select count(*) from checkpoints where file_id = $1 and kind = 'runStart') \
                         where file_id = $1",
                    )
                    .bind(&room)
                    .bind(i32::try_from(r.next_n).unwrap_or(i32::MAX))
                    .execute(&mut *tx)
                    .await
                    .map_err(failed)?;
                    let sql = format!("select {COLUMNS} from checkpoints where file_id = $1 and n = $2");
                    let made = sqlx::query(&sql)
                        .bind(&room)
                        .bind(i32::try_from(r.n).unwrap_or(i32::MAX))
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(failed)?;
                    let (checkpoint, _) = row(&made)
                        .ok_or_else(|| CheckpointError::Unavailable("the row is unreadable".into()))?;
                    Ok(Made { checkpoint, created: true, rolled: r.delete })
                }
            }
        })
    }

    fn list(&self, workspace: &str, file: &str) -> Result<Vec<Checkpoint>, CheckpointError> {
        let room = room_key(workspace, file);
        self.rt.block_on(async {
            let sql =
                format!("select {COLUMNS} from checkpoints where file_id = $1 order by n desc");
            let rows = sqlx::query(&sql)
                .bind(&room)
                .fetch_all(&self.pool)
                .await
                .map_err(failed)?;
            Ok(rows.iter().filter_map(row).map(|(c, _)| c).collect())
        })
    }
}
