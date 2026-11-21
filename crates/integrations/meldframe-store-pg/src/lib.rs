//! The Postgres adapter of the store seam (E1.2).
//!
//! One `files` row per room holds the counter `seq`; an append locks that row, so the numbers of a room
//! are gap-free and two writers (two connections, two server processes) never take the same one. The log
//! is `ops`, append-only (`migrations/0001_init.sql`, `0002_ops_key_per_actor.sql`), keyed by
//! `(file, actor, idempotency key)`. Snapshots are `snapshots` rows; the newest has the highest `seq`.
//!
//! The seam is synchronous, so each call blocks on a private runtime. Call it from a plain thread or from
//! `spawn_blocking`, never from inside an async task (tokio refuses to block there).

mod backing;
mod batches;
mod calls;
mod checkpoints;
mod review;
mod state;

pub use backing::{AnyStore, Backing};
pub use calls::{CALL_QUEUE, PgCallLog};
pub use state::{snapshot_from_state, state_json};

use meldframe_host::{Appended, NewOp, Snapshot, Store, StoreError, StoredOp};
use meldframe_protocol::{Command, SCHEMA_VERSION};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::types::Json;
use sqlx::{PgPool, Row};
use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;
use tokio::runtime::Runtime;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../../migrations");

/// A store on Postgres. Cloning is cheap and shares the connection pool: give each room its own clone.
///
/// Each `connect` builds one tokio runtime and one pool (up to 8 connections), so connect once per process
/// and clone, not once per room. Dropping the last clone drops that runtime, and tokio panics when a
/// runtime is dropped from inside an async task: drop the store from a plain thread or `spawn_blocking`.
/// The migrator owns the database: it must be empty or already managed by it (see `docs/architecture.md`).
#[derive(Debug, Clone)]
pub struct PgStore {
    // The pool is dropped before the runtime that drives it.
    pool: PgPool,
    rt: Arc<Runtime>,
    /// The caps of the batch headers (`activity_store::BatchStore`); the default is the bounds of ADR-0021.
    batch_caps: activity_store::Caps,
}

fn unavailable(e: impl Display) -> StoreError {
    StoreError::Unavailable(e.to_string())
}

/// The key of a file in the registry and the store: the workspace's length in bytes, a colon, the workspace, then
/// the file. The length prefix makes it injective (workspace `a/b` with file `c` is not workspace `a` with file
/// `b/c`), so a workspace never reaches another workspace's file by choosing its name.
pub fn room_key(workspace: &str, file: &str) -> String {
    format!("{}:{workspace}{file}", workspace.len())
}

/// The workspace and the file of a key made by [`room_key`]; `None` for any other string (including a length with
/// a sign or a leading zero, or one past the end, or inside a character). The SQL of migration 0003 follows the
/// same rule.
pub fn split_room_key(key: &str) -> Option<(&str, &str)> {
    let (len, rest) = key.split_once(':')?;
    let canonical =
        len == "0" || (len.len() <= 9 && len.starts_with(|c: char| ('1'..='9').contains(&c)));
    if !canonical || !len.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: usize = len.parse().ok()?;
    rest.is_char_boundary(n).then(|| rest.split_at(n))
}

/// The workspace a key outside the key form belongs to; the server never accepts a name with a control character.
pub const RAW_WORKSPACE: &str = "\u{1}raw";

/// What a workspace has written: ops and the bytes of their commands (as stored, in JSON).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub ops: u64,
    pub bytes: u64,
}

/// The text that starts a store error that can never succeed (the one place that writes it, and the text a
/// transport reads back with `starts_with`; the rest of the message comes from the database, after it).
pub const PERMANENT_STORE_ERROR: &str = "data refused, retrying cannot succeed";

/// A database error as a store error. The seam has no permanent-failure variant (and `meldframe-host` is at
/// its size ceiling), so a data exception (SQLSTATE class 22, such as a NUL character that `jsonb` cannot
/// hold) is `Unavailable` with a message that says retrying cannot succeed; a caller that wants to tell the
/// two apart has to read that text.
fn db_err(e: sqlx::Error) -> StoreError {
    match &e {
        sqlx::Error::Database(d) if d.code().is_some_and(|c| c.starts_with("22")) => {
            StoreError::Unavailable(format!("{PERMANENT_STORE_ERROR}: {e}"))
        }
        _ => unavailable(e),
    }
}

fn plain_name(schema: &str) -> Result<(), StoreError> {
    let plain = schema
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if schema.is_empty() || !plain {
        return Err(unavailable(format!("bad schema name {schema:?}")));
    }
    Ok(())
}

/// A sequence number as Postgres holds it (`bigint`); anything above `i64::MAX` cannot be in the log.
fn db_seq(seq: u64) -> i64 {
    i64::try_from(seq).unwrap_or(i64::MAX)
}

fn from_db_seq(seq: i64) -> u64 {
    u64::try_from(seq).unwrap_or(0)
}

impl PgStore {
    /// Connect to `url`, apply the migrations that are missing and return the store (schema `public`, or
    /// whatever the URL's search path says).
    pub fn connect(url: &str) -> Result<Self, StoreError> {
        Self::open(url, None)
    }

    /// Like [`PgStore::connect`], on a schema of its own, created when missing. Tests use it to get an
    /// empty database; a deployment can use it to keep Meldframe out of `public`.
    pub fn connect_in_schema(url: &str, schema: &str) -> Result<Self, StoreError> {
        let plain = schema
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
        if schema.is_empty() || !plain {
            return Err(unavailable(format!("bad schema name {schema:?}")));
        }
        Self::open(url, Some(schema.to_string()))
    }

    /// The same store with other caps on the batch headers: tests use small ones.
    pub fn with_batch_caps(mut self, caps: activity_store::Caps) -> Self {
        self.batch_caps = caps;
        self
    }

    /// Drop a schema made by [`PgStore::connect_in_schema`] with everything in it. For tests and tools; it
    /// opens its own connection, so close every store on that schema first.
    pub fn drop_schema(url: &str, schema: &str) -> Result<(), StoreError> {
        plain_name(schema)?;
        let rt = Runtime::new().map_err(unavailable)?;
        let options = PgConnectOptions::from_str(url).map_err(db_err)?;
        rt.block_on(async {
            let mut conn = sqlx::ConnectOptions::connect(&options).await?;
            sqlx::query(&format!("drop schema if exists \"{schema}\" cascade"))
                .execute(&mut conn)
                .await?;
            Ok::<_, sqlx::Error>(())
        })
        .map_err(db_err)
    }

    /// The ops a workspace has written and their size: read from its own rows, so it survives a restart. The size
    /// is that of the stored JSON of each command.
    pub fn workspace_usage(&self, workspace: &str) -> Result<Usage, StoreError> {
        let row = self
            .rt
            .block_on(
                sqlx::query(
                    "select count(*) as ops, coalesce(sum(o.bytes), 0)::bigint as bytes \
                     from ops o join files f on f.id = o.file_id where f.workspace_id = $1",
                )
                .bind(workspace)
                .fetch_one(&self.pool),
            )
            .map_err(db_err)?;
        Ok(Usage {
            ops: from_db_seq(row.get("ops")),
            bytes: from_db_seq(row.get("bytes")),
        })
    }

    /// The ops a workspace wrote since `since_ms` (milliseconds since the Unix epoch, compared with the time the
    /// database gave each op), for the per-minute quota of a restarted server.
    pub fn ops_since(&self, workspace: &str, since_ms: u64) -> Result<u64, StoreError> {
        let row = self
            .rt
            .block_on(
                sqlx::query(
                    "select count(*) as ops from ops o join files f on f.id = o.file_id \
                     where f.workspace_id = $1 and o.created_at >= to_timestamp($2::double precision / 1000)",
                )
                .bind(workspace)
                .bind(since_ms as f64)
                .fetch_one(&self.pool),
            )
            .map_err(db_err)?;
        Ok(from_db_seq(row.get("ops")))
    }

    fn open(url: &str, schema: Option<String>) -> Result<Self, StoreError> {
        let rt = Arc::new(Runtime::new().map_err(unavailable)?);
        let options = PgConnectOptions::from_str(url).map_err(db_err)?;
        let pool = rt.block_on(async {
            if let Some(schema) = &schema {
                let bootstrap = PgPoolOptions::new()
                    .max_connections(1)
                    .connect_with(options.clone())
                    .await?;
                sqlx::query(&format!("create schema if not exists \"{schema}\""))
                    .execute(&bootstrap)
                    .await?;
                bootstrap.close().await;
            }
            let path = schema.map(|s| format!("\"{s}\""));
            let pool = PgPoolOptions::new()
                .max_connections(8)
                .after_connect(move |conn, _| {
                    let path = path.clone();
                    Box::pin(async move {
                        if let Some(path) = path {
                            sqlx::query(&format!("set search_path to {path}"))
                                .execute(conn)
                                .await?;
                        }
                        Ok(())
                    })
                })
                .connect_with(options)
                .await?;
            // Tables without the migrator's bookkeeping come from `psql -f migrations/*.sql` (the dev seed).
            // Running the migrator on them would fail half way on "already exists", so say why up front.
            let by_hand: bool = sqlx::query_scalar(
                "select to_regclass('files') is not null and to_regclass('_sqlx_migrations') is null",
            )
            .fetch_one(&pool)
            .await?;
            if by_hand {
                return Err(sqlx::Error::Configuration(
                    "the database holds Meldframe tables that were not applied by the sqlx migrator \
                     (loaded with psql?); use an empty database or schema"
                        .into(),
                ));
            }
            MIGRATOR.run(&pool).await?;
            Ok::<_, sqlx::Error>(pool)
        });
        Ok(PgStore {
            pool: pool.map_err(db_err)?,
            rt,
            batch_caps: Default::default(),
        })
    }

    async fn append_async(&self, room: &str, op: NewOp) -> Result<Appended, StoreError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        // A key that is not in key form (a tool writing straight to the store) lives in the raw workspace.
        let (workspace, name) = split_room_key(room).unwrap_or((RAW_WORKSPACE, room));
        sqlx::query(
            "insert into files (id, schema_version, workspace_id, name) values ($1, $2, $3, $4) on conflict (id) do nothing",
        )
        .bind(room)
        .bind(SCHEMA_VERSION as i32)
        .bind(workspace)
        .bind(name)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        // The row lock is what orders the writers of one room.
        let head: i64 = sqlx::query_scalar("select seq from files where id = $1 for update")
            .bind(room)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)?;
        let first = sqlx::query(
            "select seq, command = $4 as same from ops where file_id = $1 and actor = $2 and idempotency_key = $3",
        )
        .bind(room)
        .bind(&op.actor)
        .bind(&op.key)
        .bind(Json(&op.command))
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        if let Some(row) = first {
            let seq = from_db_seq(row.get("seq"));
            if !row.get::<bool, _>("same") {
                return Err(StoreError::KeyReused { seq });
            }
            return Ok(Appended {
                seq,
                written: false,
            });
        }
        let seq = head + 1;
        sqlx::query(
            "insert into ops (file_id, seq, idempotency_key, actor, command, schema_version, bytes) values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(room)
        .bind(seq)
        .bind(&op.key)
        .bind(&op.actor)
        .bind(Json(&op.command))
        .bind(SCHEMA_VERSION as i32)
        // The size the gate counts: the compact JSON of the command.
        .bind(serde_json::to_vec(&op.command).map_or(0, |v| i32::try_from(v.len()).unwrap_or(i32::MAX)))
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query("update files set seq = $2 where id = $1")
            .bind(room)
            .bind(seq)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(Appended {
            seq: from_db_seq(seq),
            written: true,
        })
    }

    async fn save_snapshot_async(&self, room: &str, snapshot: Snapshot) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let head: Option<i64> =
            sqlx::query_scalar("select seq from files where id = $1 for update")
                .bind(room)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_err)?;
        let Some(head) = head else {
            return Err(StoreError::SnapshotAhead {
                seq: snapshot.seq,
                log: 0,
            });
        };
        if db_seq(snapshot.seq) > head {
            return Err(StoreError::SnapshotAhead {
                seq: snapshot.seq,
                log: from_db_seq(head),
            });
        }
        let latest: Option<i64> =
            sqlx::query_scalar("select max(seq) from snapshots where file_id = $1")
                .bind(room)
                .fetch_one(&mut *tx)
                .await
                .map_err(db_err)?;
        if let Some(latest) = latest.filter(|l| *l >= db_seq(snapshot.seq)) {
            return Err(StoreError::SnapshotStale {
                seq: snapshot.seq,
                latest: from_db_seq(latest),
            });
        }
        sqlx::query(
            "insert into snapshots (file_id, seq, state, schema_version) values ($1, $2, $3, $4)",
        )
        .bind(room)
        .bind(db_seq(snapshot.seq))
        .bind(Json(state_json(&snapshot)))
        .bind(SCHEMA_VERSION as i32)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }
}

impl Store for PgStore {
    fn append(&mut self, room: &str, op: NewOp) -> Result<Appended, StoreError> {
        self.rt.block_on(self.append_async(room, op))
    }

    fn find(&self, room: &str, actor: &str, key: &str) -> Result<Option<u64>, StoreError> {
        let seq: Option<i64> = self
            .rt
            .block_on(
                sqlx::query_scalar(
                    "select seq from ops where file_id = $1 and actor = $2 and idempotency_key = $3",
                )
                .bind(room)
                .bind(actor)
                .bind(key)
                .fetch_optional(&self.pool),
            )
            .map_err(db_err)?;
        Ok(seq.map(from_db_seq))
    }

    fn latest_seq(&self, room: &str) -> Result<u64, StoreError> {
        let seq: Option<i64> = self
            .rt
            .block_on(
                sqlx::query_scalar("select seq from files where id = $1")
                    .bind(room)
                    .fetch_optional(&self.pool),
            )
            .map_err(db_err)?;
        Ok(seq.map_or(0, from_db_seq))
    }

    fn ops_after(&self, room: &str, seq: u64, limit: usize) -> Result<Vec<StoredOp>, StoreError> {
        let rows = self
            .rt
            .block_on(
                sqlx::query(
                    "select seq, actor, idempotency_key, command from ops where file_id = $1 and seq > $2 order by seq limit $3",
                )
                .bind(room)
                .bind(db_seq(seq))
                .bind(i64::try_from(limit).unwrap_or(i64::MAX))
                .fetch_all(&self.pool),
            )
            .map_err(db_err)?;
        rows.into_iter()
            .map(|row| {
                let command: Json<Command> = row.try_get("command").map_err(db_err)?;
                Ok(StoredOp {
                    seq: from_db_seq(row.get("seq")),
                    actor: row.get("actor"),
                    key: row.get("idempotency_key"),
                    command: command.0,
                })
            })
            .collect()
    }

    fn save_snapshot(&mut self, room: &str, snapshot: Snapshot) -> Result<(), StoreError> {
        self.rt.block_on(self.save_snapshot_async(room, snapshot))
    }

    fn latest_snapshot(&self, room: &str) -> Result<Option<Snapshot>, StoreError> {
        let row = self
            .rt
            .block_on(
                sqlx::query(
                    "select seq, state from snapshots where file_id = $1 order by seq desc limit 1",
                )
                .bind(room)
                .fetch_optional(&self.pool),
            )
            .map_err(db_err)?;
        row.map(|row| {
            let state: Json<serde_json::Value> = row.try_get("state").map_err(db_err)?;
            snapshot_from_state(from_db_seq(row.get("seq")), state.0).map_err(|why| {
                StoreError::Unavailable(format!(
                    "{PERMANENT_STORE_ERROR}: bad snapshot state: {why}"
                ))
            })
        })
        .transpose()
    }
}
