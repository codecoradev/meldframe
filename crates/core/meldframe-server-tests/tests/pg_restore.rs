//! A room on Postgres survives a restart: snapshot plus log give the same file, and the log stays gap-free
//! when several writers append at once.

mod common;

use meldframe_host::{Gate, Op, Principal, Room, Store};
use meldframe_protocol::{Command, Node};
use meldframe_server::PgStore;
use std::collections::BTreeSet;

fn node(id: &str) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": "frame", "parentId": null}))
        .unwrap()
}

fn add(id: &str) -> Command {
    Command::AddNode {
        node: node(id),
        index: None,
    }
}

fn put(room: &mut Room<PgStore>, actor: &str, key: &str, id: &str) -> u64 {
    let who = Principal::new("ws_1", actor);
    let adm = Gate::new()
        .submit(&who, vec![Op::new(key, add(id))])
        .unwrap()
        .admitted
        .remove(0);
    room.accept(adm).unwrap().seq
}

fn tree(room: &Room<PgStore>) -> String {
    serde_json::to_string(room.nodes()).unwrap()
}

#[test]
fn every_file_restores_from_snapshot_plus_log_with_no_change() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let scratch = common::Scratch::new(&url);
    let mut before = Vec::new();
    // Three files: one from the log alone, one with a snapshot and a tail, one with exactly a snapshot.
    for (file, ops, every) in [("file_a", 4, 100), ("file_b", 7, 3), ("file_c", 6, 3)] {
        let mut r = Room::open("ws_1", file, scratch.reconnect())
            .unwrap()
            .with_snapshot_every(every);
        for i in 0..ops {
            put(&mut r, "a", &format!("k{i}"), &format!("{file}_n{i}"));
        }
        before.push((file, tree(&r), r.seq()));
    }
    // Restart: every connection is new.
    for (file, tree_before, seq) in before {
        let mut r = Room::open("ws_1", file, scratch.reconnect()).unwrap();
        assert_eq!(tree(&r), tree_before, "{file}");
        assert_eq!(r.seq(), seq, "{file}");
        // The restored room carries on from the next number, and remembers its keys.
        assert_eq!(put(&mut r, "a", "k0", &format!("{file}_n0")), 1);
        assert_eq!(put(&mut r, "a", "after", "after"), seq + 1);
    }
    let s = scratch.reconnect();
    assert_eq!(s.latest_snapshot("file_b").unwrap().unwrap().seq, 6);
    assert_eq!(s.latest_snapshot("file_a").unwrap().map(|s| s.seq), None);
}

#[test]
fn concurrent_appends_are_gap_free_and_idempotent_per_key() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let scratch = common::Scratch::new(&url);
    let store = scratch.store();
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let mut s = store.clone();
            std::thread::spawn(move || {
                (0..10)
                    .map(|i| {
                        let op = meldframe_host::NewOp {
                            actor: format!("actor{t}"),
                            key: format!("k{i}"),
                            command: add(&format!("n{t}_{i}")),
                        };
                        // Every op is sent twice: the second one must be a duplicate with the same number.
                        let first = s.append("r1", op.clone()).unwrap();
                        let again = s.append("r1", op).unwrap();
                        assert_eq!(again.seq, first.seq);
                        assert!(first.written && !again.written);
                        first.seq
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let seqs: BTreeSet<u64> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    assert_eq!(seqs, (1..=80).collect::<BTreeSet<u64>>());
    let log = store.ops_after("r1", 0, usize::MAX).unwrap();
    let logged: Vec<u64> = log.iter().map(|o| o.seq).collect();
    assert_eq!(logged, (1..=80).collect::<Vec<u64>>());
}

#[test]
fn the_same_key_sent_at_once_by_two_connections_writes_once() {
    let Some(url) = common::database_url() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let scratch = common::Scratch::new(&url);
    let store = scratch.store();
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let mut s = store.clone();
            std::thread::spawn(move || {
                let op = meldframe_host::NewOp {
                    actor: "a".into(),
                    key: "same".into(),
                    command: add("n1"),
                };
                s.append("r1", op).unwrap()
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|a| a.written).count(), 1);
    assert!(results.iter().all(|a| a.seq == 1));
    assert_eq!(store.latest_seq("r1").unwrap(), 1);
}
