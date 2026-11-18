//! E2.4: the call log in Postgres. Every entry the server logs is a row, a new process reads the newest entries
//! back, a workspace reads only its own, and a row holds no content.

mod common;

use meldframe_calls::{CallEntry, CallLog, Decision};
use meldframe_module::RiskTier;
use meldframe_server::PgCallLog;

fn entry(ws: &str, tool: &str, outcome: &str) -> CallEntry {
    CallEntry {
        at: 1_000,
        workspace: ws.into(),
        actor: "agent:x".into(),
        tool: tool.into(),
        file: "f1".into(),
        risk: Some(RiskTier::Change),
        decision: Some(Decision::Allow),
        outcome: outcome.into(),
    }
}

#[test]
fn logged_calls_are_rows_that_a_new_process_reads_back_per_workspace() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let mut log = PgCallLog::new(db.store());
    log.record(entry("ws_a", "apply_commands", "ok"));
    log.record(entry("ws_b", "get_tree", "ok"));
    let refused = CallEntry {
        risk: Some(RiskTier::Delete),
        decision: Some(Decision::Deny),
        ..entry("ws_a", "apply_commands", "forbidden")
    };
    log.record(refused.clone());
    log.record(CallEntry {
        risk: None,
        decision: None,
        ..entry("ws_a", "apply_commands", "invalid")
    });
    // The rows are written off the caller's thread: wait until they are there.
    log.flush();
    assert_eq!(log.recent(2).len(), 2, "recent reads the newest in memory");
    // A new process on the same database.
    let again = PgCallLog::new(db.reconnect());
    let a = again.of_workspace("ws_a", 10).unwrap();
    assert_eq!(
        a.iter().map(|e| e.outcome.as_str()).collect::<Vec<_>>(),
        ["ok", "forbidden", "invalid"],
        "oldest first, only ws_a"
    );
    assert_eq!(a[1], refused);
    assert_eq!(a[2].decision, None);
    assert_eq!(again.of_workspace("ws_b", 10).unwrap().len(), 1);
    assert!(again.of_workspace("ws_none", 10).unwrap().is_empty());
    assert_eq!(
        again.of_workspace("ws_a", 2).unwrap().len(),
        2,
        "the newest two"
    );
}

#[test]
fn a_server_on_postgres_keeps_its_calls_in_the_table() {
    use meldframe_host::{Op, Principal};
    use meldframe_server::{Backing, Server};
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let mut s = Server::assemble_in(Vec::new(), Backing::Postgres(db.reconnect()));
    let node =
        serde_json::from_value(serde_json::json!({"id": "n1", "type": "frame", "name": "SECRET"}))
            .unwrap();
    // An unknown tool is refused and logged; nothing reaches the room.
    let _ = s.submit_tool(
        "nope",
        "f1",
        &Principal::new("ws_a", "agent:x"),
        vec![Op::new(
            "k1",
            meldframe_protocol::Command::AddNode { node, index: None },
        )],
    );
    drop(s);
    let log = PgCallLog::new(db.reconnect());
    let mut rows = Vec::new();
    for _ in 0..200 {
        rows = log.of_workspace("ws_a", 10).unwrap();
        if !rows.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        (rows[0].tool.as_str(), rows[0].file.as_str()),
        ("nope", "f1")
    );
    assert!(!format!("{rows:?}").contains("SECRET"), "no content");
}
