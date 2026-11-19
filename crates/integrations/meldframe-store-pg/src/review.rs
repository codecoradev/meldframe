//! The critique lists in Postgres (E5.4): table `design_review_items` of migration 0006. Every method is one
//! transaction. An add and a link take a lock on the file's list first (`pg_advisory_xact_lock` on the key of the
//! file, as `room_key` makes it), so numbers are given one after the other and the limits hold when calls run at
//! once; the lock goes with the transaction.

use super::{PgStore, room_key};
use design_review::{
    Item, MAX_BATCHES, MAX_ITEMS, ReviewError, ReviewStore, Status, check_batch, check_texts,
};
use sqlx::{PgConnection, Row, postgres::PgRow};

fn failed(e: sqlx::Error) -> ReviewError {
    ReviewError::Unavailable(format!("{e}"))
}

fn item(r: &PgRow) -> Item {
    Item {
        n: u32::try_from(r.get::<i32, _>("n")).unwrap_or(0),
        text: r.get("body"),
        by: r.get("actor"),
        status: if r.get("addressed") {
            Status::Addressed
        } else {
            Status::Open
        },
        batches: r.get("batches"),
    }
}

const COLUMNS: &str = "n, body, actor, addressed, batches";

async fn lock(tx: &mut PgConnection, workspace: &str, file: &str) -> Result<(), sqlx::Error> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(room_key(workspace, file))
        .execute(tx)
        .await
        .map(|_| ())
}

impl PgStore {
    /// Run `f` in a transaction on the runtime of the store, committing when it returns `Ok`. Blocks, like the other
    /// `PgStore` methods: call it off the async runtime.
    fn review_tx<T>(
        &self,
        f: impl AsyncFnOnce(&mut PgConnection) -> Result<T, ReviewError>,
    ) -> Result<T, ReviewError> {
        self.rt.block_on(async {
            let mut tx = self.pool.begin().await.map_err(failed)?;
            let out = f(&mut *tx).await?;
            tx.commit().await.map_err(failed)?;
            Ok(out)
        })
    }
}

impl ReviewStore for PgStore {
    fn add(
        &self,
        workspace: &str,
        file: &str,
        actor: &str,
        texts: &[String],
    ) -> Result<Vec<Item>, ReviewError> {
        let texts = check_texts(texts)?;
        self.review_tx(async |tx| {
            lock(tx, workspace, file).await.map_err(failed)?;
            let last: Option<i32> =
                sqlx::query_scalar("select max(n) from design_review_items where workspace_id = $1 and file = $2")
                    .bind(workspace)
                    .bind(file)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(failed)?;
            let last = u32::try_from(last.unwrap_or(0)).unwrap_or(0);
            if last as usize + texts.len() > MAX_ITEMS {
                return Err(ReviewError::Full { limit: MAX_ITEMS });
            }
            let mut added = Vec::new();
            for (text, n) in texts.into_iter().zip(last + 1..) {
                sqlx::query(
                    "insert into design_review_items (workspace_id, file, n, body, actor) values ($1, $2, $3, $4, $5)",
                )
                .bind(workspace)
                .bind(file)
                .bind(i32::try_from(n).unwrap_or(i32::MAX))
                .bind(&text)
                .bind(actor)
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
                added.push(Item {
                    n,
                    text,
                    by: actor.to_owned(),
                    status: Status::Open,
                    batches: Vec::new(),
                });
            }
            Ok(added)
        })
    }

    fn list(&self, workspace: &str, file: &str) -> Result<Vec<Item>, ReviewError> {
        let sql = format!(
            "select {COLUMNS} from design_review_items where workspace_id = $1 and file = $2 order by n"
        );
        let rows = self
            .rt
            .block_on(
                sqlx::query(&sql)
                    .bind(workspace)
                    .bind(file)
                    .fetch_all(&self.pool),
            )
            .map_err(failed)?;
        Ok(rows.iter().map(item).collect())
    }

    fn link(&self, workspace: &str, file: &str, n: u32, batch: &str) -> Result<Item, ReviewError> {
        check_batch(batch)?;
        self.review_tx(async |tx| {
            let sql = format!(
                "select {COLUMNS} from design_review_items where workspace_id = $1 and file = $2 and n = $3 for update"
            );
            let row = sqlx::query(&sql)
                .bind(workspace)
                .bind(file)
                .bind(i32::try_from(n).unwrap_or(0))
                .fetch_optional(&mut *tx)
                .await
                .map_err(failed)?
                .ok_or(ReviewError::ItemNotFound(n))?;
            let mut found = item(&row);
            if found.batches.iter().any(|b| b == batch) {
                return Ok(found);
            }
            if found.batches.len() >= MAX_BATCHES {
                return Err(ReviewError::Full { limit: MAX_BATCHES });
            }
            sqlx::query(
                "update design_review_items set batches = array_append(batches, $4) where workspace_id = $1 and file = $2 and n = $3",
            )
            .bind(workspace)
            .bind(file)
            .bind(i32::try_from(n).unwrap_or(0))
            .bind(batch)
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
            found.batches.push(batch.to_owned());
            Ok(found)
        })
    }

    fn mark(
        &self,
        workspace: &str,
        file: &str,
        n: u32,
        status: Status,
    ) -> Result<Item, ReviewError> {
        let sql = format!(
            "update design_review_items set addressed = $4 where workspace_id = $1 and file = $2 and n = $3 returning {COLUMNS}"
        );
        let row = self
            .rt
            .block_on(
                sqlx::query(&sql)
                    .bind(workspace)
                    .bind(file)
                    .bind(i32::try_from(n).unwrap_or(0))
                    .bind(status == Status::Addressed)
                    .fetch_optional(&self.pool),
            )
            .map_err(failed)?
            .ok_or(ReviewError::ItemNotFound(n))?;
        Ok(item(&row))
    }
}
