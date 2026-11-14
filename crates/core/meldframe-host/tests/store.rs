//! The store seam: an append-only log per room with actor, idempotency key and sequence number, and
//! snapshots. Tests run against the in-memory adapter through the trait.

mod common;

use common::{add, node};
use meldframe_host::{MemoryStore, NewOp, Snapshot, Store, StoreError};

fn new_op(actor: &str, key: &str, id: &str) -> NewOp {
    NewOp {
        actor: actor.into(),
        key: key.into(),
        command: add(id),
    }
}

fn store() -> Box<dyn Store> {
    Box::new(MemoryStore::new())
}

#[test]
fn sequence_numbers_start_at_one_and_count_per_room() {
    let mut s = store();
    assert_eq!(s.append("r1", new_op("a", "k1", "n1")).unwrap().seq, 1);
    assert_eq!(s.append("r1", new_op("a", "k2", "n2")).unwrap().seq, 2);
    assert_eq!(s.append("r2", new_op("a", "k1", "n1")).unwrap().seq, 1);
    assert_eq!(s.latest_seq("r1").unwrap(), 2);
    assert_eq!(s.latest_seq("r2").unwrap(), 1);
    assert_eq!(s.latest_seq("none").unwrap(), 0);
}

#[test]
fn a_retried_key_returns_the_same_sequence_number_and_writes_nothing() {
    let mut s = store();
    let first = s.append("r1", new_op("a", "k1", "n1")).unwrap();
    s.append("r1", new_op("a", "k2", "n2")).unwrap();
    assert!(first.written);
    let again = s.append("r1", new_op("a", "k1", "n1")).unwrap();
    assert_eq!(again.seq, first.seq);
    assert!(!again.written);
    assert_eq!(s.latest_seq("r1").unwrap(), 2);
    let log = s.ops_after("r1", 0).unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(
        serde_json::to_value(&log[0].command).unwrap(),
        serde_json::to_value(add("n1")).unwrap()
    );
}

#[test]
fn a_key_belongs_to_one_actor_in_one_room() {
    let mut s = store();
    s.append("r1", new_op("a", "k1", "n1")).unwrap();
    let b = s.append("r1", new_op("b", "k1", "n2")).unwrap();
    assert!(b.written);
    assert_eq!(b.seq, 2);
    assert_eq!(s.find("r1", "a", "k1").unwrap(), Some(1));
    assert_eq!(s.find("r1", "b", "k1").unwrap(), Some(2));
    assert_eq!(s.find("r2", "a", "k1").unwrap(), None);
    assert_eq!(s.find("r1", "a", "nope").unwrap(), None);
}

#[test]
fn ops_after_returns_the_log_in_order_with_actor_and_key() {
    let mut s = store();
    for (i, (actor, key)) in [("a", "k1"), ("b", "k1"), ("a", "k2")].iter().enumerate() {
        s.append("r1", new_op(actor, key, &format!("n{i}")))
            .unwrap();
    }
    let all = s.ops_after("r1", 0).unwrap();
    let rows: Vec<(u64, &str, &str)> = all
        .iter()
        .map(|o| (o.seq, o.actor.as_str(), o.key.as_str()))
        .collect();
    assert_eq!(rows, [(1, "a", "k1"), (2, "b", "k1"), (3, "a", "k2")]);
    let tail = s.ops_after("r1", 2).unwrap();
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].seq, 3);
    assert!(s.ops_after("r1", 3).unwrap().is_empty());
    assert!(s.ops_after("none", 0).unwrap().is_empty());
}

#[test]
fn there_is_no_snapshot_until_one_is_saved_and_the_newest_wins() {
    let mut s = store();
    assert!(s.latest_snapshot("r1").unwrap().is_none());
    for i in 0..4 {
        s.append("r1", new_op("a", &format!("k{i}"), &format!("n{i}")))
            .unwrap();
    }
    let snap = |seq| Snapshot {
        seq,
        nodes: vec![node("root", "frame", None)],
    };
    s.save_snapshot("r1", snap(2)).unwrap();
    s.save_snapshot("r1", snap(4)).unwrap();
    let got = s.latest_snapshot("r1").unwrap().unwrap();
    assert_eq!(got.seq, 4);
    assert_eq!(got.nodes.len(), 1);
    assert!(s.latest_snapshot("r2").unwrap().is_none());
}

#[test]
fn a_snapshot_ahead_of_the_log_or_older_than_the_last_is_refused() {
    let mut s = store();
    s.append("r1", new_op("a", "k1", "n1")).unwrap();
    s.append("r1", new_op("a", "k2", "n2")).unwrap();
    let snap = |seq| Snapshot { seq, nodes: vec![] };
    assert_eq!(
        s.save_snapshot("r1", snap(3)),
        Err(StoreError::SnapshotAhead { seq: 3, log: 2 })
    );
    s.save_snapshot("r1", snap(2)).unwrap();
    assert_eq!(
        s.save_snapshot("r1", snap(1)),
        Err(StoreError::SnapshotStale { seq: 1, latest: 2 })
    );
    assert_eq!(s.latest_snapshot("r1").unwrap().unwrap().seq, 2);
}

#[test]
fn a_snapshot_never_changes_the_log() {
    let mut s = store();
    s.append("r1", new_op("a", "k1", "n1")).unwrap();
    s.save_snapshot(
        "r1",
        Snapshot {
            seq: 1,
            nodes: vec![],
        },
    )
    .unwrap();
    assert_eq!(s.ops_after("r1", 0).unwrap().len(), 1);
    assert_eq!(s.append("r1", new_op("a", "k2", "n2")).unwrap().seq, 2);
}

#[test]
fn a_key_reused_for_a_different_command_is_refused_and_writes_nothing() {
    let mut s = store();
    let first = s.append("r1", new_op("a", "k1", "n1")).unwrap();
    let err = s.append("r1", new_op("a", "k1", "other")).unwrap_err();
    assert_eq!(err, StoreError::KeyReused { seq: first.seq });
    assert_eq!(s.latest_seq("r1").unwrap(), 1);
}

#[test]
fn a_snapshot_must_be_newer_than_the_latest_and_a_failed_save_keeps_the_old_one() {
    let mut s = store();
    for (k, id) in [("k1", "n1"), ("k2", "n2")] {
        s.append("r1", new_op("a", k, id)).unwrap();
    }
    let snap = |seq| Snapshot {
        seq,
        nodes: vec![node("n1", "frame", None)],
    };
    s.save_snapshot("r1", snap(1)).unwrap();
    assert_eq!(
        s.save_snapshot("r1", snap(1)).unwrap_err(),
        StoreError::SnapshotStale { seq: 1, latest: 1 }
    );
    assert_eq!(s.latest_snapshot("r1").unwrap().unwrap().seq, 1);
    s.save_snapshot("r1", snap(2)).unwrap();
    assert_eq!(s.latest_snapshot("r1").unwrap().unwrap().seq, 2);
}

#[test]
fn a_snapshot_restores_with_the_log_after_it() {
    let mut s = store();
    for (k, id) in [("k1", "n1"), ("k2", "n2"), ("k3", "n3")] {
        s.append("r1", new_op("a", k, id)).unwrap();
    }
    s.save_snapshot(
        "r1",
        Snapshot {
            seq: 2,
            nodes: vec![node("n1", "frame", None), node("n2", "frame", None)],
        },
    )
    .unwrap();
    let snap = s.latest_snapshot("r1").unwrap().unwrap();
    let tail = s.ops_after("r1", snap.seq).unwrap();
    assert_eq!(snap.nodes.len(), 2);
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].seq, 3);
}
