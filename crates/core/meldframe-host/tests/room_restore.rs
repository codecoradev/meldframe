//! A room restores from its store: snapshot plus log give the same file, and the rules still hold.

mod common;

use common::{add, admit, op, refusal};
use meldframe_host::{MemoryStore, Op, Room, RoomError, SnapshotWarning, Store, StoreError};
use std::{cell::Cell, rc::Rc};

fn put<S: Store>(room: &mut Room<S>, actor: &str, o: Op) -> Result<u64, RoomError> {
    let (o, p) = admit(actor, o);
    room.accept(o, p).map(|a| a.seq)
}

fn tree(room: &Room<MemoryStore>) -> String {
    serde_json::to_string(room.nodes()).unwrap()
}

fn restart(room: Room<MemoryStore>) -> Room<MemoryStore> {
    Room::open("ws_1", "file_1", room.into_store()).unwrap()
}

#[test]
fn a_restart_from_the_log_alone_gives_the_same_file() {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    for i in 0..5 {
        put(&mut r, "a", op(&format!("k{i}"), add(&format!("n{i}")))).unwrap();
    }
    let before = tree(&r);
    let r = restart(r);
    assert_eq!(tree(&r), before);
    assert_eq!(r.seq(), 5);
}

#[test]
fn a_snapshot_is_saved_every_n_ops_and_the_restart_uses_it() {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new())
        .unwrap()
        .with_snapshot_every(3);
    for i in 0..7 {
        put(&mut r, "a", op(&format!("k{i}"), add(&format!("n{i}")))).unwrap();
    }
    assert_eq!(r.store().latest_snapshot("file_1").unwrap().unwrap().seq, 6);
    let before = tree(&r);
    let mut r = restart(r);
    assert_eq!(tree(&r), before);
    assert_eq!(put(&mut r, "a", op("k7", add("n7"))).unwrap(), 8);
}

#[test]
fn keys_stay_remembered_across_a_restart_and_a_snapshot() {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new())
        .unwrap()
        .with_snapshot_every(2);
    for i in 0..4 {
        put(&mut r, "a", op(&format!("k{i}"), add(&format!("n{i}")))).unwrap();
    }
    let mut r = restart(r);
    // A resend of an old key is a duplicate with its first number.
    let (o, p) = admit("a", op("k0", add("n0")));
    let again = r.accept(o, p).unwrap();
    assert_eq!((again.seq, again.written), (1, false));
    // An op that depends on a key from before the snapshot is accepted.
    let dep = op("k9", add("n9")).with_deps(["k0"]);
    assert_eq!(put(&mut r, "a", dep).unwrap(), 5);
}

#[test]
fn if_seq_older_than_the_snapshot_is_treated_as_a_conflict() {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new())
        .unwrap()
        .with_snapshot_every(2);
    for i in 0..2 {
        put(&mut r, "a", op(&format!("k{i}"), add(&format!("n{i}")))).unwrap();
    }
    let mut r = restart(r);
    let rm = meldframe_protocol::Command::RemoveNode { id: "n0".into() };
    let mut gate = meldframe_host::Gate::with_policy(|_, _| meldframe_module::Policy::Auto);
    let who = meldframe_host::Principal::new("ws_1", "a");
    let a = gate
        .submit(&who, vec![op("k5", rm).with_if_seq(0)])
        .unwrap()
        .admitted
        .remove(0);
    let err = refusal(r.accept(a.op, a.permit).unwrap_err());
    assert_eq!(err.code(), "conflict");
}

#[test]
fn a_log_that_does_not_replay_is_reported_not_hidden() {
    let mut store = MemoryStore::new();
    let bad = meldframe_protocol::Command::RemoveNode { id: "ghost".into() };
    store
        .append(
            "file_1",
            meldframe_host::NewOp {
                actor: "a".into(),
                key: "k1".into(),
                command: bad,
            },
        )
        .unwrap();
    match Room::open("ws_1", "file_1", store) {
        Err(RoomError::Corrupt { seq }) => assert_eq!(seq, 1),
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

/// A store whose snapshot saves fail while the flag in `.1` is set. The op was already written when
/// the failure happens.
struct NoSnapshots(MemoryStore, Rc<Cell<bool>>);

impl Store for NoSnapshots {
    fn append(
        &mut self,
        room: &str,
        op: meldframe_host::NewOp,
    ) -> Result<meldframe_host::Appended, meldframe_host::StoreError> {
        self.0.append(room, op)
    }
    fn find(
        &self,
        room: &str,
        actor: &str,
        key: &str,
    ) -> Result<Option<u64>, meldframe_host::StoreError> {
        self.0.find(room, actor, key)
    }
    fn latest_seq(&self, room: &str) -> Result<u64, meldframe_host::StoreError> {
        self.0.latest_seq(room)
    }
    fn ops_after(
        &self,
        room: &str,
        seq: u64,
    ) -> Result<Vec<meldframe_host::StoredOp>, meldframe_host::StoreError> {
        self.0.ops_after(room, seq)
    }
    fn save_snapshot(
        &mut self,
        room: &str,
        snapshot: meldframe_host::Snapshot,
    ) -> Result<(), meldframe_host::StoreError> {
        if self.1.get() {
            return Err(meldframe_host::StoreError::Unavailable("disk full".into()));
        }
        self.0.save_snapshot(room, snapshot)
    }
    fn latest_snapshot(
        &self,
        room: &str,
    ) -> Result<Option<meldframe_host::Snapshot>, meldframe_host::StoreError> {
        self.0.latest_snapshot(room)
    }
}

#[test]
fn a_failed_snapshot_does_not_turn_an_accepted_op_into_an_error() {
    let mut r = Room::open(
        "ws_1",
        "file_1",
        NoSnapshots(MemoryStore::new(), Rc::new(Cell::new(true))),
    )
    .unwrap()
    .with_snapshot_every(1);
    let (o, p) = admit("a", op("k1", add("n1")));
    let done = r
        .accept(o, p)
        .expect("the op is written, so it is accepted");
    assert_eq!((done.seq, done.written), (1, true));
    assert_eq!(r.seq(), 1);
    assert_eq!(r.snapshot_failures(), 1);
}

#[test]
fn a_failed_snapshot_is_reported_with_its_seq_and_cause() {
    let mut r = Room::open(
        "ws_1",
        "file_1",
        NoSnapshots(MemoryStore::new(), Rc::new(Cell::new(true))),
    )
    .unwrap()
    .with_snapshot_every(2);
    assert_eq!(r.snapshot_warning(), None);
    put(&mut r, "a", op("k1", add("n1"))).unwrap();
    assert_eq!(r.snapshot_warning(), None, "no save was tried at seq 1");
    put(&mut r, "a", op("k2", add("n2"))).unwrap();
    assert_eq!(
        r.snapshot_warning(),
        Some(&SnapshotWarning {
            seq: 2,
            error: StoreError::Unavailable("disk full".into())
        })
    );
    // A later failure replaces the warning: it names the newest snapshot that is missing.
    put(&mut r, "a", op("k3", add("n3"))).unwrap();
    put(&mut r, "a", op("k4", add("n4"))).unwrap();
    assert_eq!(r.snapshot_warning().map(|w| w.seq), Some(4));
    assert_eq!(r.snapshot_failures(), 2);
}

#[test]
fn a_later_successful_snapshot_clears_the_warning() {
    let failing = Rc::new(Cell::new(true));
    let store = NoSnapshots(MemoryStore::new(), failing.clone());
    let mut r = Room::open("ws_1", "file_1", store)
        .unwrap()
        .with_snapshot_every(1);
    put(&mut r, "a", op("k1", add("n1"))).unwrap();
    assert!(r.snapshot_warning().is_some());
    failing.set(false);
    put(&mut r, "a", op("k2", add("n2"))).unwrap();
    assert_eq!(r.snapshot_warning(), None);
    assert_eq!(r.snapshot_failures(), 1, "the counter is a total, it stays");
}

#[test]
fn a_restart_starts_from_the_snapshot_not_from_an_empty_file() {
    // The snapshot holds a node that no op in the log created: only a room that loads it can show it.
    let mut store = MemoryStore::new();
    for i in 0..3 {
        let (o, _) = admit("a", op(&format!("k{i}"), add(&format!("n{i}"))));
        store
            .append(
                "file_1",
                meldframe_host::NewOp {
                    actor: "a".into(),
                    key: o.key.clone(),
                    command: o.command.clone(),
                },
            )
            .unwrap();
    }
    store
        .save_snapshot(
            "file_1",
            meldframe_host::Snapshot {
                seq: 2,
                nodes: vec![common::node("from_snapshot", "frame", None)],
            },
        )
        .unwrap();
    let r = Room::open("ws_1", "file_1", store).unwrap();
    let ids: Vec<&str> = r.nodes().iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["from_snapshot", "n2"]);
}
