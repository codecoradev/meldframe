//! The cases. Each takes a fresh store through the trait object, so an adapter proves the seam, not itself.

use meldframe_host::{NewOp, Snapshot, Store, StoreError, StoredOp};
use meldframe_protocol::{Command, Node};

fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

fn add(id: &str) -> Command {
    Command::AddNode {
        node: node(id, "frame", None),
        index: None,
    }
}

fn new_op(actor: &str, key: &str, id: &str) -> NewOp {
    NewOp {
        actor: actor.into(),
        key: key.into(),
        command: add(id),
    }
}

pub fn sequence_numbers_start_at_one_and_count_per_room(s: &mut dyn Store) {
    assert_eq!(s.append("r1", new_op("a", "k1", "n1")).unwrap().seq, 1);
    assert_eq!(s.append("r1", new_op("a", "k2", "n2")).unwrap().seq, 2);
    assert_eq!(s.append("r2", new_op("a", "k1", "n1")).unwrap().seq, 1);
    assert_eq!(s.latest_seq("r1").unwrap(), 2);
    assert_eq!(s.latest_seq("r2").unwrap(), 1);
    assert_eq!(s.latest_seq("none").unwrap(), 0);
}

pub fn a_retried_key_returns_the_same_sequence_number_and_writes_nothing(s: &mut dyn Store) {
    let first = s.append("r1", new_op("a", "k1", "n1")).unwrap();
    s.append("r1", new_op("a", "k2", "n2")).unwrap();
    assert!(first.written);
    let again = s.append("r1", new_op("a", "k1", "n1")).unwrap();
    assert_eq!(again.seq, first.seq);
    assert!(!again.written);
    assert_eq!(s.latest_seq("r1").unwrap(), 2);
    let log = s.ops_after("r1", 0, usize::MAX).unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(
        serde_json::to_value(&log[0].command).unwrap(),
        serde_json::to_value(add("n1")).unwrap()
    );
}

pub fn a_key_belongs_to_one_actor_in_one_room(s: &mut dyn Store) {
    s.append("r1", new_op("a", "k1", "n1")).unwrap();
    let b = s.append("r1", new_op("b", "k1", "n2")).unwrap();
    assert!(b.written);
    assert_eq!(b.seq, 2);
    assert_eq!(s.find("r1", "a", "k1").unwrap(), Some(1));
    assert_eq!(s.find("r1", "b", "k1").unwrap(), Some(2));
    assert_eq!(s.find("r2", "a", "k1").unwrap(), None);
    assert_eq!(s.find("r1", "a", "nope").unwrap(), None);
}

pub fn ops_after_returns_the_log_in_order_with_actor_and_key(s: &mut dyn Store) {
    for (i, (actor, key)) in [("a", "k1"), ("b", "k1"), ("a", "k2")].iter().enumerate() {
        s.append("r1", new_op(actor, key, &format!("n{i}")))
            .unwrap();
    }
    let all = s.ops_after("r1", 0, usize::MAX).unwrap();
    let rows: Vec<(u64, &str, &str)> = all
        .iter()
        .map(|o| (o.seq, o.actor.as_str(), o.key.as_str()))
        .collect();
    assert_eq!(rows, [(1, "a", "k1"), (2, "b", "k1"), (3, "a", "k2")]);
    let tail = s.ops_after("r1", 2, usize::MAX).unwrap();
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].seq, 3);
    assert!(s.ops_after("r1", 3, usize::MAX).unwrap().is_empty());
    assert!(s.ops_after("none", 0, usize::MAX).unwrap().is_empty());
}

pub fn a_limit_smaller_than_the_tail_returns_the_first_page_in_order(s: &mut dyn Store) {
    for i in 0..5 {
        s.append("r1", new_op("a", &format!("k{i}"), &format!("n{i}")))
            .unwrap();
    }
    let seqs = |ops: Vec<StoredOp>| ops.iter().map(|o| o.seq).collect::<Vec<_>>();
    assert_eq!(seqs(s.ops_after("r1", 0, 2).unwrap()), [1, 2]);
    assert_eq!(seqs(s.ops_after("r1", 2, 2).unwrap()), [3, 4]);
    // The last page is short, and a limit that is large enough returns the whole tail.
    assert_eq!(seqs(s.ops_after("r1", 4, 2).unwrap()), [5]);
    assert_eq!(seqs(s.ops_after("r1", 1, 100).unwrap()), [2, 3, 4, 5]);
    // A limit of 0 returns nothing; a caller must not loop on it.
    assert!(s.ops_after("r1", 0, 0).unwrap().is_empty());
}

pub fn replay_through_pages_equals_replay_in_one_go(s: &mut dyn Store) {
    for i in 0..7 {
        s.append("r1", new_op("a", &format!("k{i}"), &format!("n{i}")))
            .unwrap();
    }
    let whole = s.ops_after("r1", 0, usize::MAX).unwrap();
    for limit in [1, 2, 3, 7, 8] {
        let mut paged = Vec::new();
        let mut after = 0;
        loop {
            let page = s.ops_after("r1", after, limit).unwrap();
            if page.is_empty() {
                break;
            }
            assert!(page.len() <= limit);
            after = page.last().unwrap().seq;
            paged.extend(page);
        }
        let key = |ops: &[StoredOp]| {
            ops.iter()
                .map(|o| (o.seq, o.actor.clone(), o.key.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(key(&paged), key(&whole), "limit {limit}");
    }
}

pub fn there_is_no_snapshot_until_one_is_saved_and_the_newest_wins(s: &mut dyn Store) {
    assert!(s.latest_snapshot("r1").unwrap().is_none());
    for i in 0..4 {
        s.append("r1", new_op("a", &format!("k{i}"), &format!("n{i}")))
            .unwrap();
    }
    let snap = |seq| Snapshot {
        seq,
        nodes: vec![node("root", "frame", None)],
        variables: Default::default(),
    };
    s.save_snapshot("r1", snap(2)).unwrap();
    s.save_snapshot("r1", snap(4)).unwrap();
    let got = s.latest_snapshot("r1").unwrap().unwrap();
    assert_eq!(got.seq, 4);
    assert_eq!(got.nodes.len(), 1);
    assert!(s.latest_snapshot("r2").unwrap().is_none());
}

pub fn a_snapshot_ahead_of_the_log_or_older_than_the_last_is_refused(s: &mut dyn Store) {
    s.append("r1", new_op("a", "k1", "n1")).unwrap();
    s.append("r1", new_op("a", "k2", "n2")).unwrap();
    let snap = |seq| Snapshot {
        seq,
        nodes: vec![],
        variables: Default::default(),
    };
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

pub fn a_snapshot_never_changes_the_log(s: &mut dyn Store) {
    s.append("r1", new_op("a", "k1", "n1")).unwrap();
    s.save_snapshot(
        "r1",
        Snapshot {
            seq: 1,
            nodes: vec![],
            variables: Default::default(),
        },
    )
    .unwrap();
    assert_eq!(s.ops_after("r1", 0, usize::MAX).unwrap().len(), 1);
    assert_eq!(s.append("r1", new_op("a", "k2", "n2")).unwrap().seq, 2);
}

pub fn a_key_reused_for_a_different_command_is_refused_and_writes_nothing(s: &mut dyn Store) {
    let first = s.append("r1", new_op("a", "k1", "n1")).unwrap();
    let err = s.append("r1", new_op("a", "k1", "other")).unwrap_err();
    assert_eq!(err, StoreError::KeyReused { seq: first.seq });
    assert_eq!(s.latest_seq("r1").unwrap(), 1);
}

pub fn a_snapshot_must_be_newer_than_the_latest_and_a_failed_save_keeps_the_old_one(
    s: &mut dyn Store,
) {
    for (k, id) in [("k1", "n1"), ("k2", "n2")] {
        s.append("r1", new_op("a", k, id)).unwrap();
    }
    let snap = |seq| Snapshot {
        seq,
        nodes: vec![node("n1", "frame", None)],
        variables: Default::default(),
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

pub fn a_snapshot_restores_with_the_log_after_it(s: &mut dyn Store) {
    for (k, id) in [("k1", "n1"), ("k2", "n2"), ("k3", "n3")] {
        s.append("r1", new_op("a", k, id)).unwrap();
    }
    s.save_snapshot(
        "r1",
        Snapshot {
            seq: 2,
            nodes: vec![node("n1", "frame", None), node("n2", "frame", None)],
            variables: Default::default(),
        },
    )
    .unwrap();
    let snap = s.latest_snapshot("r1").unwrap().unwrap();
    let tail = s.ops_after("r1", snap.seq, usize::MAX).unwrap();
    assert_eq!(snap.nodes.len(), 2);
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].seq, 3);
}

/// What goes into the log comes back equal, including fields a database could round or reorder.
pub fn a_command_comes_back_from_the_log_unchanged(s: &mut dyn Store) {
    let n: Node = serde_json::from_value(serde_json::json!({
        "id": "n1", "type": "frame", "parentId": null, "x": 10.5, "y": -3.25, "w": 120.0, "name": "caf\u{e9} \"quoted\""
    }))
    .unwrap();
    let command = Command::AddNode {
        node: n,
        index: Some(0),
    };
    s.append(
        "r1",
        NewOp {
            actor: "a".into(),
            key: "k1".into(),
            command: command.clone(),
        },
    )
    .unwrap();
    let got = s.ops_after("r1", 0, 10).unwrap();
    assert_eq!(
        serde_json::to_value(&got[0].command).unwrap(),
        serde_json::to_value(&command).unwrap()
    );
}

/// A snapshot comes back with the same nodes in the same order.
pub fn a_snapshot_comes_back_unchanged(s: &mut dyn Store) {
    s.append("r1", new_op("a", "k1", "n1")).unwrap();
    let nodes = vec![
        node("n2", "frame", None),
        node("n1", "frame", None),
        node("c", "frame", Some("n1")),
    ];
    s.save_snapshot(
        "r1",
        Snapshot {
            seq: 1,
            nodes: nodes.clone(),
            variables: Default::default(),
        },
    )
    .unwrap();
    let got = s.latest_snapshot("r1").unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&got.nodes).unwrap(),
        serde_json::to_value(&nodes).unwrap()
    );
}

/// Variables are kept with the snapshot, and a variable command comes back from the log unchanged (E3.3).
pub fn variables_come_back_with_the_snapshot_and_the_log(s: &mut dyn Store) {
    let vars: meldframe_protocol::Variables = serde_json::from_value(serde_json::json!({
        "collections": [{"id": "t", "name": "theme", "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
            "values": {"light": "#ffffff", "dark": {"var": "bg2"}}}]
    }))
    .unwrap();
    let command: Command = serde_json::from_value(serde_json::json!(
        {"type": "setVariableValue", "id": "bg", "mode": "dark", "value": {"var": "bg2"}}
    ))
    .unwrap();
    let op = NewOp {
        actor: "a".into(),
        key: "k1".into(),
        command: command.clone(),
    };
    s.append("r1", op).unwrap();
    s.save_snapshot(
        "r1",
        Snapshot {
            seq: 1,
            nodes: vec![],
            variables: vars.clone(),
        },
    )
    .unwrap();
    assert_eq!(s.latest_snapshot("r1").unwrap().unwrap().variables, vars);
    assert_eq!(
        serde_json::to_value(&s.ops_after("r1", 0, 10).unwrap()[0].command).unwrap(),
        serde_json::to_value(&command).unwrap()
    );
}
