//! The activity log seam: one entry per accepted op, none for a retry or a refusal.

use meldframe_host::{Admission, Gate, MemoryStore, Op, Principal, Room};
use meldframe_module::{Policy, RiskTier};
use meldframe_protocol::Command;
use meldframe_rooms::{
    ActivityEntry, ActivityError, ActivityKind, ActivityLog, Logged, MemoryActivityLog,
    accept_logged,
};

fn add(id: &str) -> Command {
    let node = serde_json::from_value(serde_json::json!({"id": id, "type": "frame"})).unwrap();
    Command::AddNode { node, index: None }
}

fn remove(id: &str) -> Command {
    Command::RemoveNode { id: id.into() }
}

fn op(key: &str, command: Command) -> Op {
    Op::new(key, command)
}

fn room() -> Room<MemoryStore> {
    Room::open("ws_1", "file_1", MemoryStore::new()).unwrap()
}

/// Admit with a policy that lets a delete through, so the log can be checked for a Delete tier.
fn admit(actor: &str, o: Op) -> Admission {
    let mut gate = Gate::with_policy(|_, _| Policy::Auto);
    gate.submit(&Principal::new("ws_1", actor), vec![o])
        .unwrap()
        .admitted
        .remove(0)
}

fn try_put(
    r: &mut Room<MemoryStore>,
    log: &mut impl ActivityLog,
    actor: &str,
    at: u64,
    o: Op,
) -> Result<Logged, meldframe_host::RoomError> {
    accept_logged(r, log, admit(actor, o), at)
}

fn put(r: &mut Room<MemoryStore>, log: &mut impl ActivityLog, actor: &str, at: u64, o: Op) -> bool {
    try_put(r, log, actor, at, o).is_ok()
}

fn entries(log: &MemoryActivityLog) -> Vec<ActivityEntry> {
    log.entries_after("file_1", 0, usize::MAX).unwrap()
}

#[test]
fn an_accepted_op_records_who_what_when_tier_and_sequence_number() {
    let (mut r, mut log) = (room(), MemoryActivityLog::new());
    assert!(put(&mut r, &mut log, "user:a", 100, op("k1", add("n1"))));
    assert!(put(
        &mut r,
        &mut log,
        "agent:b",
        250,
        op("k2", remove("n1"))
    ));
    let rows: Vec<_> = entries(&log)
        .into_iter()
        .map(|e| (e.seq, e.actor, e.kind, e.node_id, e.at, e.tier))
        .collect();
    assert_eq!(
        rows,
        [
            (
                1,
                "user:a".into(),
                ActivityKind::AddNode,
                "n1".into(),
                100,
                RiskTier::Change
            ),
            (
                2,
                "agent:b".into(),
                ActivityKind::RemoveNode,
                "n1".into(),
                250,
                RiskTier::Delete
            ),
        ]
    );
}

#[test]
fn every_command_kind_has_a_kind_and_a_node_id() {
    let (mut r, mut log) = (room(), MemoryActivityLog::new());
    assert!(put(&mut r, &mut log, "a", 1, op("k1", add("f1"))));
    assert!(put(&mut r, &mut log, "a", 2, op("k2", add("n1"))));
    let cmds = [
        Command::UpdateNode {
            id: "n1".into(),
            patch: Default::default(),
        },
        Command::Reparent {
            id: "n1".into(),
            parent_id: Some("f1".into()),
            pos: None,
        },
        Command::Reorder {
            id: "n1".into(),
            before: None,
            after: None,
            pos: None,
        },
    ];
    for (i, c) in cmds.into_iter().enumerate() {
        assert!(
            put(&mut r, &mut log, "a", 3, op(&format!("m{i}"), c)),
            "command {i}"
        );
    }
    let rows: Vec<_> = entries(&log)
        .into_iter()
        .map(|e| (e.kind, e.node_id))
        .collect();
    assert_eq!(rows[2], (ActivityKind::UpdateNode, "n1".into()));
    assert_eq!(rows[3], (ActivityKind::Reparent, "n1".into()));
    assert_eq!(rows[4], (ActivityKind::Reorder, "n1".into()));
}

#[test]
fn a_retried_key_records_nothing_new() {
    let (mut r, mut log) = (room(), MemoryActivityLog::new());
    let first = try_put(&mut r, &mut log, "a", 10, op("k1", add("n1"))).unwrap();
    assert!(first.accepted.written && first.log_error.is_none());
    let again = try_put(&mut r, &mut log, "a", 99, op("k1", add("n1"))).unwrap();
    assert!(!again.accepted.written && again.log_error.is_none());
    let rows = entries(&log);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].at, 10);
}

#[test]
fn a_refused_op_records_nothing() {
    let (mut r, mut log) = (room(), MemoryActivityLog::new());
    assert!(put(&mut r, &mut log, "a", 1, op("k1", add("n1"))));
    // The document core refuses a duplicate id.
    assert!(!put(&mut r, &mut log, "a", 2, op("k2", add("n1"))));
    // A dependency that was never applied.
    let dep = op("k3", add("n3")).with_deps(["nope"]);
    assert!(!put(&mut r, &mut log, "a", 3, dep));
    // A conflict: n1 changed at 1, the caller saw nothing (ifSeq 0).
    let stale = op("k4", remove("n1")).with_if_seq(0);
    assert!(!put(&mut r, &mut log, "a", 4, stale));
    // A reused key with another command.
    assert!(!put(&mut r, &mut log, "a", 5, op("k1", add("n9"))));
    assert_eq!(entries(&log).len(), 1);
}

/// A log that cannot be written.
#[derive(Debug)]
struct Broken;

impl ActivityLog for Broken {
    fn record(&mut self, _: &str, _: ActivityEntry) -> Result<(), ActivityError> {
        Err(ActivityError::Unavailable("disk full".into()))
    }
    fn entries_after(
        &self,
        _: &str,
        _: u64,
        _: usize,
    ) -> Result<Vec<ActivityEntry>, ActivityError> {
        Ok(Vec::new())
    }
}

#[test]
fn a_failed_log_write_does_not_fail_an_op_that_is_already_in_the_store() {
    let mut r = room();
    let adm = admit("a", op("k1", add("n1")));
    let done = accept_logged(&mut r, &mut Broken, adm, 1).unwrap();
    assert_eq!(done.accepted.seq, 1);
    assert_eq!(r.seq(), 1);
    assert_eq!(
        done.log_error,
        Some(ActivityError::Unavailable("disk full".into()))
    );
}

#[test]
fn entries_are_kept_per_room_and_read_in_pages() {
    let mut log = MemoryActivityLog::new();
    let entry = |seq| ActivityEntry {
        seq,
        actor: "a".into(),
        kind: ActivityKind::AddNode,
        node_id: format!("n{seq}"),
        at: seq,
        tier: RiskTier::Change,
    };
    for seq in 1..=5 {
        log.record("r1", entry(seq)).unwrap();
    }
    log.record("r2", entry(1)).unwrap();
    let seqs = |v: Vec<ActivityEntry>| v.into_iter().map(|e| e.seq).collect::<Vec<_>>();
    assert_eq!(seqs(log.entries_after("r1", 0, 2).unwrap()), [1, 2]);
    assert_eq!(seqs(log.entries_after("r1", 2, 2).unwrap()), [3, 4]);
    assert_eq!(seqs(log.entries_after("r1", 4, 2).unwrap()), [5]);
    assert_eq!(seqs(log.entries_after("r2", 0, 10).unwrap()), [1]);
    assert!(log.entries_after("none", 0, 10).unwrap().is_empty());
}

#[test]
fn kinds_have_stable_names_that_read_back() {
    let names: Vec<&str> = ActivityKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(
        names,
        [
            "add_node",
            "update_node",
            "remove_node",
            "reparent",
            "reorder",
            "variable"
        ]
    );
    for k in ActivityKind::ALL {
        assert_eq!(ActivityKind::parse(k.as_str()), Some(k));
    }
    assert_eq!(ActivityKind::parse("from_a_newer_version"), None);
}

#[test]
fn the_memory_log_refuses_an_entry_that_is_not_newer_than_the_last() {
    let mut log = MemoryActivityLog::new();
    let entry = |seq| ActivityEntry {
        seq,
        actor: "a".into(),
        kind: ActivityKind::AddNode,
        node_id: "n".into(),
        at: seq,
        tier: RiskTier::Change,
    };
    log.record("r1", entry(2)).unwrap();
    assert_eq!(
        log.record("r1", entry(2)).unwrap_err(),
        ActivityError::OutOfOrder { seq: 2, last: 2 }
    );
    assert_eq!(
        log.record("r1", entry(1)).unwrap_err(),
        ActivityError::OutOfOrder { seq: 1, last: 2 }
    );
    log.record("r2", entry(1)).unwrap();
    assert_eq!(log.entries_after("r1", 0, 10).unwrap().len(), 1);
}

#[test]
fn a_lost_entry_shows_as_a_gap_in_the_sequence_numbers() {
    // The log is best effort: when one write fails the op stays accepted and the entry is missing; the
    // gap in `seq` shows it, and the store's op log is the source to repair from.
    struct FailsOnce(MemoryActivityLog, bool);
    impl ActivityLog for FailsOnce {
        fn record(&mut self, room: &str, e: ActivityEntry) -> Result<(), ActivityError> {
            if !self.1 {
                self.1 = true;
                return Err(ActivityError::Unavailable("blip".into()));
            }
            self.0.record(room, e)
        }
        fn entries_after(
            &self,
            room: &str,
            seq: u64,
            limit: usize,
        ) -> Result<Vec<ActivityEntry>, ActivityError> {
            self.0.entries_after(room, seq, limit)
        }
    }
    let (mut r, mut log) = (room(), FailsOnce(MemoryActivityLog::new(), false));
    let one = try_put(&mut r, &mut log, "a", 1, op("k1", add("n1"))).unwrap();
    assert!(one.log_error.is_some());
    let two = try_put(&mut r, &mut log, "a", 2, op("k2", add("n2"))).unwrap();
    assert!(two.log_error.is_none());
    let seqs: Vec<u64> = log
        .entries_after("file_1", 0, 10)
        .unwrap()
        .iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(seqs, [2]);
    assert_eq!(r.seq(), 2);
}
