//! The batch headers in Postgres (E5.1, ADR-0021): tables `batches` and `batch_files` of migration 0007. Every method
//! that writes is one transaction that takes `pg_advisory_xact_lock` on the workspace's batches first, with a key
//! prefix of its own (`batches:`; the design review lists use `design_review:`), so numbers and `rev`s are given one
//! after the other and the caps of the file and of the workspace hold when calls run at once; the lock goes with the
//! transaction. The counters are read and updated under that lock from the counter row of the file, never from
//! `max(n)`, and the answer is the row read back inside the transaction that wrote it. The count of open headers
//! (`unreviewedCount`) is a counter of the same row, not counted from the headers. The per-actor cap is the one indexed count, over at most
//! 2,000 rows, and only for a new header.

use super::{PgStore, from_db_seq, room_key};
use activity_store::{
    ActivityError, BatchStore, NoteOutcome, Page, Query, Recorded, Reviewed, cap_reached,
    check_actor, check_batch, check_query, merge, new_header,
};
use meldframe_activity::{Header, NewBatch, Note, Review};
use sqlx::{PgConnection, Row, postgres::PgRow};

/// A database error as the seam's: a foreign key violation (a header for a file that has no ops yet) can never
/// succeed as it is, so it is `Invalid`; anything else is `Unavailable`, which a caller may retry. Only `Display`
/// of the error is ever kept.
fn failed(e: sqlx::Error) -> ActivityError {
    if e.as_database_error().and_then(|d| d.code()).as_deref() == Some("23503") {
        return ActivityError::Invalid(
            "a header is written after its ops: this file has none yet".to_owned(),
        );
    }
    ActivityError::Unavailable(format!("{e}"))
}

const COLUMNS: &str = "n, rev, actor, key_prefix, tool, commands, written, atomic, first_seq, last_seq, at_ms, \
                       note, review, reviewed_by, reviewed_at_ms";

fn count(r: &PgRow, col: &str) -> u32 {
    u32::try_from(r.get::<i32, _>(col)).unwrap_or(0)
}

fn header(r: &PgRow) -> Header {
    Header {
        n: count(r, "n"),
        rev: from_db_seq(r.get("rev")),
        actor: r.get("actor"),
        key_prefix: r.get("key_prefix"),
        tool: r.get("tool"),
        commands: count(r, "commands"),
        written: count(r, "written"),
        atomic: r.get("atomic"),
        first_seq: from_db_seq(r.get("first_seq")),
        last_seq: from_db_seq(r.get("last_seq")),
        at_ms: from_db_seq(r.get("at_ms")),
        // A stored note was checked by the server; the app layer's cleaning is the same rule, so a row edited by
        // hand cannot bring hidden text into a header.
        note: r
            .get::<Option<String>, _>("note")
            .and_then(|t| Note::clean(&t)),
        review: Review::parse(r.get::<&str, _>("review")).unwrap_or_default(),
        reviewed_by: r.get("reviewed_by"),
        reviewed_at_ms: r.get::<Option<i64>, _>("reviewed_at_ms").map(from_db_seq),
    }
}

fn db_i32(n: u32) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

fn db_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// The lock of a workspace's batch headers, held until the transaction ends.
async fn lock(tx: &mut PgConnection, workspace: &str) -> Result<(), sqlx::Error> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("batches:{}:{workspace}", workspace.len()))
        .execute(tx)
        .await
        .map(|_| ())
}

/// How many headers of the room are not accepted: the counter row is kept under the lock, no count runs.
async fn unreviewed(tx: &mut PgConnection, room: &str) -> Result<u32, ActivityError> {
    let n: Option<i32> =
        sqlx::query_scalar("select open_headers from batch_files where file_id = $1")
            .bind(room)
            .fetch_optional(tx)
            .await
            .map_err(failed)?;
    Ok(n.map_or(0, |n| u32::try_from(n).unwrap_or(0)))
}

/// The header `n` of the room as it is now.
async fn read(tx: &mut PgConnection, room: &str, n: i32) -> Result<Header, ActivityError> {
    let sql = format!("select {COLUMNS} from batches where file_id = $1 and n = $2");
    let row = sqlx::query(&sql)
        .bind(room)
        .bind(n)
        .fetch_one(tx)
        .await
        .map_err(failed)?;
    Ok(header(&row))
}

/// Take the next `rev` of the room: the counter row is read and raised under the lock.
async fn take_rev(tx: &mut PgConnection, room: &str) -> Result<i64, ActivityError> {
    sqlx::query_scalar(
        "update batch_files set next_rev = next_rev + 1 where file_id = $1 returning next_rev - 1",
    )
    .bind(room)
    .fetch_one(tx)
    .await
    .map_err(failed)
}

impl PgStore {
    /// Run `f` in a transaction on the runtime of the store, committing when it returns `Ok`. Blocks, like the other
    /// `PgStore` methods: call it off the async runtime.
    fn batch_tx<T>(
        &self,
        f: impl AsyncFnOnce(&mut PgConnection) -> Result<T, ActivityError>,
    ) -> Result<T, ActivityError> {
        self.rt.block_on(async {
            let mut tx = self.pool.begin().await.map_err(failed)?;
            let out = f(&mut *tx).await?;
            tx.commit().await.map_err(failed)?;
            Ok(out)
        })
    }
}

impl BatchStore for PgStore {
    fn record(
        &self,
        workspace: &str,
        file: &str,
        batch: &NewBatch,
    ) -> Result<Recorded, ActivityError> {
        check_batch(batch)?;
        let room = room_key(workspace, file);
        self.batch_tx(async |tx| {
            lock(tx, workspace).await.map_err(failed)?;
            let sql = format!(
                "select {COLUMNS} from batches where file_id = $1 and actor = $2 and key_prefix = $3"
            );
            let found = sqlx::query(&sql)
                .bind(&room)
                .bind(&batch.actor)
                .bind(&batch.key_prefix)
                .fetch_optional(&mut *tx)
                .await
                .map_err(failed)?;
            if let Some(row) = found {
                let mut h = header(&row);
                let (changed, note) = merge(&mut h, batch);
                if changed {
                    let rev = take_rev(tx, &room).await?;
                    // The trigger lets only these columns change, and the note only from null (a stored note is kept).
                    sqlx::query(
                        "update batches set commands = $3, written = $4, last_seq = $5, note = coalesce(note, $6), rev = $7 \
                         where file_id = $1 and n = $2",
                    )
                    .bind(&room)
                    .bind(db_i32(h.n))
                    .bind(db_i32(h.commands))
                    .bind(db_i32(h.written))
                    .bind(db_i64(h.last_seq))
                    .bind(h.note.as_ref().map(Note::as_str))
                    .bind(rev)
                    .execute(&mut *tx)
                    .await
                    .map_err(failed)?;
                }
                let header = read(tx, &room, db_i32(h.n)).await?;
                let unreviewed = unreviewed(tx, &room).await?;
                return Ok(Recorded { header, created: false, changed, note, unreviewed });
            }
            // A new header: the counter row of the file, then the caps, then the number.
            sqlx::query(
                "insert into batch_files (file_id, workspace_id) values ($1, $2) on conflict (file_id) do nothing",
            )
            .bind(&room)
            .bind(workspace)
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
            let (n, headers): (i32, i32) =
                sqlx::query_as("select next_n, headers from batch_files where file_id = $1")
                    .bind(&room)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(failed)?;
            let mine: i64 =
                sqlx::query_scalar("select count(*) from batches where file_id = $1 and actor = $2")
                    .bind(&room)
                    .bind(&batch.actor)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(failed)?;
            let in_workspace: i64 = sqlx::query_scalar(
                "select coalesce(sum(headers), 0)::bigint from batch_files where workspace_id = $1",
            )
            .bind(workspace)
            .fetch_one(&mut *tx)
            .await
            .map_err(failed)?;
            let used = |n: i64| usize::try_from(n).unwrap_or(usize::MAX);
            if let Some(full) =
                cap_reached(&self.batch_caps, used(mine), used(headers.into()), used(in_workspace))
            {
                return Err(full);
            }
            let rev = take_rev(tx, &room).await?;
            sqlx::query(
                "insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, \
                 note, rev, first_seq, last_seq, at_ms) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
            )
            .bind(&room)
            .bind(workspace)
            .bind(n)
            .bind(&batch.actor)
            .bind(&batch.key_prefix)
            .bind(&batch.tool)
            .bind(db_i32(batch.commands))
            .bind(db_i32(new_header(0, 0, batch).written))
            .bind(batch.atomic)
            .bind(batch.note.as_ref().map(Note::as_str))
            .bind(rev)
            .bind(db_i64(batch.first_seq))
            .bind(db_i64(batch.last_seq))
            .bind(db_i64(batch.at_ms))
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
            sqlx::query("update batch_files set next_n = next_n + 1, headers = headers + 1, open_headers = open_headers + 1 \
                 where file_id = $1")
                .bind(&room)
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
            let header = read(tx, &room, n).await?;
            let unreviewed = unreviewed(tx, &room).await?;
            let note = if batch.note.is_some() { NoteOutcome::Kept } else { NoteOutcome::None };
            Ok(Recorded { header, created: true, changed: true, note, unreviewed })
        })
    }

    fn list(&self, workspace: &str, file: &str, query: &Query) -> Result<Page, ActivityError> {
        check_query(query)?;
        let room = room_key(workspace, file);
        self.rt.block_on(async {
            // One snapshot for the page and the count.
            let mut tx = self.pool.begin().await.map_err(failed)?;
            sqlx::query("set transaction isolation level repeatable read")
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
            let sql = format!(
                "select {COLUMNS} from batches where file_id = $1 and ($2::int is null or n < $2) \
                 and ($3::int is null or n > $3) order by n desc limit $4"
            );
            let rows = sqlx::query(&sql)
                .bind(&room)
                .bind(query.before.map(db_i32))
                .bind(query.after.map(db_i32))
                .bind(db_i64(query.limit as u64 + 1))
                .fetch_all(&mut *tx)
                .await
                .map_err(failed)?;
            let unreviewed = unreviewed(&mut tx, &room).await?;
            let has_more = rows.len() > query.limit;
            let headers = rows.iter().take(query.limit).map(header).collect();
            Ok(Page {
                headers,
                has_more,
                unreviewed,
            })
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
        let room = room_key(workspace, file);
        let num = i32::try_from(n).map_err(|_| ActivityError::NotFound(n))?;
        self.batch_tx(async |tx| {
            lock(tx, workspace).await.map_err(failed)?;
            let exists: Option<i32> =
                sqlx::query_scalar("select n from batches where file_id = $1 and n = $2")
                    .bind(&room)
                    .bind(num)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(failed)?;
            if exists.is_none() {
                return Err(ActivityError::NotFound(n));
            }
            let current = read(tx, &room, num).await?;
            let changed = current.review != state;
            if changed {
                let rev = take_rev(tx, &room).await?;
                // The counter of open headers moves when a header enters or leaves `accepted`.
                let delta = i32::from(state.is_open()) - i32::from(current.review.is_open());
                sqlx::query("update batch_files set open_headers = open_headers + $2 where file_id = $1")
                    .bind(&room)
                    .bind(delta)
                    .execute(&mut *tx)
                    .await
                    .map_err(failed)?;
                sqlx::query(
                    "update batches set review = $3, reviewed_by = $4, reviewed_at_ms = $5, rev = $6 \
                     where file_id = $1 and n = $2",
                )
                .bind(&room)
                .bind(num)
                .bind(state.as_str())
                .bind(by)
                .bind(db_i64(at_ms))
                .bind(rev)
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
            }
            let header = read(tx, &room, num).await?;
            let unreviewed = unreviewed(tx, &room).await?;
            Ok(Reviewed { header, changed, unreviewed })
        })
    }
}
