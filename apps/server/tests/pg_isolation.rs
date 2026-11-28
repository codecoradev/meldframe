//! Schemas keep stores apart, a database the migrator did not create is refused, and data that can never be
//! stored fails without leaving a gap.

#[path = "rooms_common/mod.rs"]
mod common;

use meldframe_host::{NewOp, Store, StoreError};
use meldframe_protocol::Command;
use meldframe_server::PgStore;

fn add(id: &str, name: &str) -> Command {
    let node = serde_json::from_value(
        serde_json::json!({"id": id, "type": "frame", "parentId": null, "name": name}),
    )
    .unwrap();
    Command::AddNode { node, index: None }
}

fn op(key: &str, command: Command) -> NewOp {
    NewOp {
        actor: "a".into(),
        key: key.into(),
        command,
    }
}

#[test]
fn a_store_sees_only_its_own_schema_not_another_one_and_not_public() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let a = common::Scratch::new(&url);
    let b = common::Scratch::new(&url);
    let mut sa = a.store();
    sa.append("shared", op("k1", add("n1", "x"))).unwrap();
    assert_eq!(a.store().latest_seq("shared").unwrap(), 1);
    assert_eq!(b.store().latest_seq("shared").unwrap(), 0);
    // A store on `public` (the default schema) is another place again. It gets a database of its own, so the
    // sqlx tables it creates do not land in the test database (the psql-based dev seed would then fail).
    let db = format!("iso_{}", common::unique_schema());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let admin = |sql: String| {
        rt.block_on(async {
            use sqlx::Executor;
            let pool = sqlx::PgPool::connect(&url).await.unwrap();
            pool.execute(sql.as_str()).await.unwrap();
        })
    };
    admin(format!("create database \"{db}\""));
    let own_url = format!("{}/{db}", url.rsplit_once('/').unwrap().0);
    let mut public = PgStore::connect(&own_url).expect("connect to public");
    public.append("shared", op("k1", add("n1", "x"))).unwrap();
    assert_eq!(public.latest_seq("shared").unwrap(), 1);
    // Same room name, other schema: nothing is shared. A schema store in that database sees no `public` table.
    let other = PgStore::connect_in_schema(&own_url, "other").unwrap();
    assert_eq!(other.latest_seq("shared").unwrap(), 0);
    drop((public, other));
    admin(format!("drop database \"{db}\" with (force)"));
    assert_eq!(a.store().latest_seq("shared").unwrap(), 1);
}

#[test]
fn a_schema_made_by_hand_without_the_migrator_is_refused_with_a_clear_error() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    // What `psql -f migrations/*.sql` leaves behind: the tables, no `_sqlx_migrations`.
    let schema = common::unique_schema();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use sqlx::Executor;
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        pool.execute(format!("create schema \"{schema}\"").as_str())
            .await
            .unwrap();
        pool.execute(format!("create table \"{schema}\".files (id text primary key)").as_str())
            .await
            .unwrap();
    });
    let err = PgStore::connect_in_schema(&url, &schema).unwrap_err();
    let StoreError::Unavailable(why) = err else {
        panic!("expected Unavailable, got {err:?}");
    };
    assert!(why.contains("not applied by the sqlx migrator"), "{why}");
    PgStore::drop_schema(&url, &schema).unwrap();
}

#[test]
fn data_the_database_can_never_hold_fails_loudly_and_leaves_no_gap() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let scratch = common::Scratch::new(&url);
    let mut s = scratch.store();
    // jsonb cannot hold U+0000.
    let err = s.append("r1", op("bad", add("n1", "a\u{0}b"))).unwrap_err();
    let StoreError::Unavailable(why) = err else {
        panic!("expected a store error, got {err:?}");
    };
    assert!(why.contains("retrying cannot succeed"), "{why}");
    assert_eq!(s.latest_seq("r1").unwrap(), 0);
    assert_eq!(s.append("r1", op("good", add("n2", "ok"))).unwrap().seq, 1);
}
