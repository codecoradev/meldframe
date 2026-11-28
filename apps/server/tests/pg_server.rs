//! A restarted server restores every file from Postgres, through the server composition.

#[path = "rooms_common/mod.rs"]
mod common;

use meldframe_host::{Op, Principal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Backing, Server};

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

fn add(key: &str, id: &str) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": id, "type": "frame"})).unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

fn tree(s: &Server, file: &str) -> String {
    s.with_room("ws_1", file, |r| serde_json::to_string(r.nodes()).unwrap())
        .unwrap()
}

#[test]
fn a_restarted_server_restores_every_file_with_no_change() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let scratch = common::Scratch::new(&url);
    let w = Principal::new("ws_1", "agent:test");
    let boot = |store| Server::assemble_in(vec![Box::new(Painter)], Backing::Postgres(store));
    let mut first = boot(scratch.reconnect());
    // 520 ops pass the default snapshot interval (500), so the restore uses a snapshot plus a tail.
    for i in 0..520 {
        first
            .submit_tool(
                "paint",
                "file_a",
                &w,
                vec![add(&format!("k{i}"), &format!("a{i}"))],
            )
            .unwrap();
    }
    first
        .submit_tool("paint", "file_b", &w, vec![add("k0", "b0")])
        .unwrap();
    let before = (tree(&first, "file_a"), tree(&first, "file_b"));
    let seq_a = first.open("ws_1", "file_a").unwrap();
    drop(first);
    use meldframe_host::Store;
    let snap = scratch.reconnect().latest_snapshot("4:ws_1file_a").unwrap();
    assert!(
        snap.is_some_and(|s| s.seq < seq_a),
        "restore must use a snapshot plus a tail"
    );

    let mut second = boot(scratch.reconnect());
    assert_eq!(second.open("ws_1", "file_a").unwrap(), seq_a);
    second.open("ws_1", "file_b").unwrap();
    assert_eq!((tree(&second, "file_a"), tree(&second, "file_b")), before);
    // The restored file carries on from the next number and still knows its keys.
    let again = second
        .submit_tool("paint", "file_a", &w, vec![add("k0", "a0")])
        .unwrap();
    assert_eq!((again[0].seq, again[0].written), (1, false));
    let next = second
        .submit_tool("paint", "file_a", &w, vec![add("new", "new")])
        .unwrap();
    assert_eq!(next[0].seq, seq_a + 1);
    // Files stay apart in the database too.
    assert_eq!(second.open("ws_1", "file_b").unwrap(), 1);
}

#[test]
fn after_a_restart_another_workspace_still_gets_an_empty_file() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let scratch = common::Scratch::new(&url);
    let boot = || {
        Server::assemble_in(
            vec![Box::new(Painter)],
            Backing::Postgres(scratch.reconnect()),
        )
    };
    let w1 = Principal::new("ws_1", "agent:test");
    let w2 = Principal::new("ws_2", "agent:test");
    let mut first = boot();
    first
        .submit_tool("paint", "f", &w1, vec![add("k1", "secret")])
        .unwrap();
    drop(first);
    let mut second = boot();
    assert_eq!(second.open("ws_2", "f").unwrap(), 0);
    second
        .submit_tool("paint", "f", &w2, vec![add("k1", "mine")])
        .unwrap();
    assert_eq!(second.open("ws_1", "f").unwrap(), 1);
    let ids = |ws: &str| {
        second
            .with_room(ws, "f", |r| {
                r.nodes().iter().map(|n| n.id.clone()).collect::<Vec<_>>()
            })
            .unwrap()
    };
    assert_eq!(ids("ws_1"), ["secret"]);
    assert_eq!(ids("ws_2"), ["mine"]);
}
