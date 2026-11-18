//! E2.3: every stored file belongs to one workspace, in a column and not only in its key. The rows of the three
//! tables (files, ops, snapshots) are told apart by it, and the usage of a workspace is read from its own rows.

mod common;

use meldframe_host::{NewOp, Snapshot, Store};
use meldframe_protocol::Command;
use meldframe_server::room_key;
use sqlx::Row;

fn add(id: &str) -> Command {
    let node = serde_json::from_value(
        serde_json::json!({"id": id, "type": "frame", "parentId": null, "name": "Card"}),
    )
    .unwrap();
    Command::AddNode { node, index: None }
}

fn op(key: &str, id: &str) -> NewOp {
    NewOp {
        actor: "a".into(),
        key: key.into(),
        command: add(id),
    }
}

#[test]
fn the_workspace_is_a_column_of_files_and_the_other_tables_follow_it() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let mut s = db.store();
    // The same file name in two workspaces, one of them with a name that is not ASCII.
    let (a, b) = (room_key("ws_a", "f"), room_key("wörkspace", "f"));
    s.append(&a, op("k1", "n1")).unwrap();
    s.append(&a, op("k2", "n2")).unwrap();
    s.append(&b, op("k1", "n1")).unwrap();
    s.save_snapshot(
        &a,
        Snapshot {
            seq: 2,
            nodes: vec![],
        },
    )
    .unwrap();

    let url = common::database_url().unwrap();
    let count = |sql: String| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            use sqlx::postgres::PgConnectOptions;
            use std::str::FromStr;
            let opts = PgConnectOptions::from_str(&url)
                .unwrap()
                .options([("search_path", db.schema.as_str())]);
            let mut c = sqlx::ConnectOptions::connect(&opts).await.unwrap();
            let row = sqlx::query(&sql).fetch_one(&mut c).await.unwrap();
            row.get::<i64, _>(0)
        })
    };
    for (ws, files, ops, snaps) in [
        ("ws_a", 1, 2, 1),
        ("wörkspace", 1, 1, 0),
        ("ws_none", 0, 0, 0),
    ] {
        assert_eq!(
            count(format!(
                "select count(*) from files where workspace_id = '{ws}'"
            )),
            files,
            "{ws} files"
        );
        assert_eq!(
            count(format!(
                "select count(*) from ops o join files f on f.id = o.file_id where f.workspace_id = '{ws}'"
            )),
            ops,
            "{ws} ops"
        );
        assert_eq!(
            count(format!(
                "select count(*) from snapshots s join files f on f.id = s.file_id where f.workspace_id = '{ws}'"
            )),
            snaps,
            "{ws} snapshots"
        );
    }
    assert_eq!(
        count("select count(*) from files where name = 'f'".into()),
        2
    );
}

#[test]
fn usage_is_read_per_workspace_from_its_own_rows() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let mut s = db.store();
    s.append(&room_key("ws_a", "f"), op("k1", "n1")).unwrap();
    s.append(&room_key("ws_a", "g"), op("k1", "n1")).unwrap();
    s.append(&room_key("ws_b", "f"), op("k1", "n1")).unwrap();
    let a = s.workspace_usage("ws_a").unwrap();
    let b = s.workspace_usage("ws_b").unwrap();
    assert_eq!(a.ops, 2);
    assert_eq!(b.ops, 1);
    // The figure is the compact JSON the gate counts, not the text Postgres prints for jsonb.
    let one = serde_json::to_vec(&add("n1")).unwrap().len() as u64;
    assert_eq!((a.bytes, b.bytes), (2 * one, one));
    assert_eq!(
        s.workspace_usage("ws_none").unwrap(),
        meldframe_server::Usage::default()
    );
}

#[test]
fn the_migration_backfills_the_columns_of_files_written_before_it() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let schema = common::unique_schema();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use sqlx::postgres::PgConnectOptions;
        use sqlx::{ConnectOptions, Executor};
        use std::str::FromStr;
        let mut admin = PgConnectOptions::from_str(&url)
            .unwrap()
            .connect()
            .await
            .unwrap();
        admin
            .execute(format!("create schema \"{schema}\"").as_str())
            .await
            .unwrap();
        let opts = PgConnectOptions::from_str(&url)
            .unwrap()
            .options([("search_path", schema.as_str())]);
        let mut c = opts.connect().await.unwrap();
        for sql in [
            include_str!("../../../../migrations/0001_init.sql"),
            include_str!("../../../../migrations/0002_ops_key_per_actor.sql"),
        ] {
            c.execute(sql).await.unwrap();
        }
        // Keys as the server wrote them, one with a multi-byte workspace, and one that is not in key form.
        for id in [
            "4:ws_af1",
            "10:wörkspacef2",
            "4:ws_b/a:b",
            "legacy",
            "0:x",
            "01:ab",
            "99999999999:ab",
            "9:wör",
            "3:wör",
            "2:wör",
            "7:abc",
        ] {
            sqlx::query("insert into files (id, schema_version) values ($1, 1)")
                .bind(id)
                .execute(&mut c)
                .await
                .unwrap();
        }
        c.execute(include_str!(
            "../../../../migrations/0003_workspace_column.sql"
        ))
        .await
        .unwrap();
        let rows = sqlx::query("select id, workspace_id, name from files order by id")
            .fetch_all(&mut c)
            .await
            .unwrap();
        let got: Vec<(String, String, String)> = rows
            .iter()
            .map(|r| (r.get("id"), r.get("workspace_id"), r.get("name")))
            .collect();
        let raw = meldframe_server::RAW_WORKSPACE;
        let want = [
            ("4:ws_af1", "ws_a", "f1"),
            ("4:ws_b/a:b", "ws_b", "/a:b"),
            ("10:wörkspacef2", "wörkspace", "f2"),
            ("legacy", raw, "legacy"),
            ("0:x", "", "x"),
            // Not in key form: a leading zero, a length that does not fit an int, a length past the end, a
            // length inside a character. Each is kept whole instead of aborting the migration.
            ("01:ab", raw, "01:ab"),
            ("99999999999:ab", raw, "99999999999:ab"),
            ("9:wör", raw, "9:wör"),
            ("3:wör", "wö", "r"),
            ("2:wör", raw, "2:wör"),
            ("7:abc", raw, "7:abc"),
        ];
        let mut want: Vec<_> = want
            .iter()
            .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
            .collect();
        want.sort();
        let mut got = got;
        got.sort();
        assert_eq!(got, want);
        admin
            .execute(format!("drop schema \"{schema}\" cascade").as_str())
            .await
            .unwrap();
    });
}

#[test]
fn the_storage_and_total_counters_survive_a_restart() {
    use meldframe_host::{Op, Principal};
    use meldframe_module::{Module, Registrar, RiskTier};
    use meldframe_server::{Backing, Server};
    struct Tools;
    impl Module for Tools {
        fn name(&self) -> &'static str {
            "tools"
        }
        fn register(&self, r: &mut Registrar) {
            r.agent_tool("paint", "Paint a node.", RiskTier::Change);
        }
    }
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let node = |id: &str| -> Op { Op::new(id, add(id)) };
    let who = Principal::new("ws_a", "agent:x");
    let make =
        || {
            Server::assemble_in(vec![Box::new(Tools)], Backing::Postgres(db.reconnect()))
                .with_quotas(10_000, 1_500, || 0)
        };
    let mut s = make();
    let mut written = 0;
    let err = loop {
        match s.submit_tool("paint", "f", &who, vec![node(&format!("n{written}"))]) {
            Ok(_) => written += 1,
            Err(e) => break e,
        }
        assert!(written < 50, "the storage limit never hit");
    };
    assert_eq!(err.refusal().map(|r| r.code()), Some("quota_storage"));
    assert!(written >= 1);
    drop(s);
    // A new process on the same database: the workspace is still full, another one is not.
    let mut s = make();
    let err = s
        .submit_tool("paint", "f", &who, vec![node("again")])
        .unwrap_err();
    assert_eq!(err.refusal().map(|r| r.code()), Some("quota_storage"));
    s.submit_tool(
        "paint",
        "f",
        &Principal::new("ws_b", "agent:x"),
        vec![node("n0")],
    )
    .unwrap();
}

#[test]
fn a_key_splits_only_in_the_one_canonical_form() {
    use meldframe_server::{room_key, split_room_key};
    for (ws, f) in [("ws", "f"), ("", "x"), ("wörk", "ß/1"), ("a:b", ":c")] {
        assert_eq!(
            split_room_key(&room_key(ws, f)),
            Some((ws, f)),
            "{ws:?} {f:?}"
        );
    }
    for bad in [
        "x",
        "01:ab",
        "+1:ab",
        "-1:ab",
        "1 :ab",
        ":ab",
        "99999999999:ab",
        "3:ab",
        "1:ö",
    ] {
        assert_eq!(split_room_key(bad), None, "{bad:?}");
    }
}

#[test]
fn the_ops_of_this_minute_survive_a_restart() {
    use meldframe_host::{Op, Principal};
    use meldframe_module::{Module, Registrar, RiskTier};
    use meldframe_server::{Backing, Server};
    struct Tools;
    impl Module for Tools {
        fn name(&self) -> &'static str {
            "tools"
        }
        fn register(&self, r: &mut Registrar) {
            r.agent_tool("paint", "Paint a node.", RiskTier::Change);
        }
    }
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    // One fixed instant for both servers: every op below is written after it, so all of them fall in its minute.
    // The database stamps the ops with its own clock, so stay clear of the first seconds of a minute, where a
    // database clock a little behind would put them in the minute before.
    let wall = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    };
    if wall() % 60_000 < 5_000 {
        std::thread::sleep(std::time::Duration::from_millis(5_000));
    }
    let t0 = wall();
    let node = |id: &str| -> Op { Op::new(id, add(id)) };
    let who = Principal::new("ws_a", "agent:x");
    let make =
        || {
            Server::assemble_in(vec![Box::new(Tools)], Backing::Postgres(db.reconnect()))
                .with_quotas(3, 1 << 30, move || t0)
        };
    let mut s = make();
    for i in 0..3 {
        s.submit_tool("paint", "f", &who, vec![node(&format!("n{i}"))])
            .unwrap();
    }
    drop(s);
    // A new process: the workspace already sent its three ops of this minute, another workspace has not.
    let mut s = make();
    let err = s
        .submit_tool("paint", "f", &who, vec![node("again")])
        .unwrap_err();
    assert_eq!(
        err.refusal().map(|r| r.code()),
        Some("quota_ops_per_minute")
    );
    s.submit_tool(
        "paint",
        "f",
        &Principal::new("ws_b", "agent:x"),
        vec![node("n0")],
    )
    .unwrap();
    // The store counts from the start of a minute: none of these ops is older than t0's minute, none is
    // newer than now.
    let pg = db.reconnect();
    assert_eq!(pg.ops_since("ws_a", t0 / 60_000 * 60_000).unwrap(), 3);
    assert_eq!(pg.ops_since("ws_a", t0 + 3_600_000).unwrap(), 0);
}
