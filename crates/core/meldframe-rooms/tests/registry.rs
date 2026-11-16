//! The room registry: open is idempotent, close frees a slot, the number of open rooms is bounded, a room
//! is restored from the store on open, and every write goes through the registry.

use meldframe_host::{
    Admission, Appended, Gate, MemoryStore, NewOp, Op, Principal, RoomError, Snapshot, Store,
    StoreError, StoredOp,
};
use meldframe_protocol::Command;
use meldframe_rooms::{ActivityLog, MemoryActivityLog, RegistryError, Rooms};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A handle on one durable store: every room opened over it, before or after a close, sees the same log.
#[derive(Debug, Clone, Default)]
struct Shared(Rc<RefCell<MemoryStore>>);

impl Store for Shared {
    fn append(&mut self, room: &str, op: NewOp) -> Result<Appended, StoreError> {
        self.0.borrow_mut().append(room, op)
    }
    fn find(&self, room: &str, actor: &str, key: &str) -> Result<Option<u64>, StoreError> {
        self.0.borrow().find(room, actor, key)
    }
    fn latest_seq(&self, room: &str) -> Result<u64, StoreError> {
        self.0.borrow().latest_seq(room)
    }
    fn ops_after(&self, room: &str, seq: u64, limit: usize) -> Result<Vec<StoredOp>, StoreError> {
        self.0.borrow().ops_after(room, seq, limit)
    }
    fn save_snapshot(&mut self, room: &str, snapshot: Snapshot) -> Result<(), StoreError> {
        self.0.borrow_mut().save_snapshot(room, snapshot)
    }
    fn latest_snapshot(&self, room: &str) -> Result<Option<Snapshot>, StoreError> {
        self.0.borrow().latest_snapshot(room)
    }
}

fn add(id: &str) -> Command {
    let node = serde_json::from_value(serde_json::json!({"id": id, "type": "frame"})).unwrap();
    Command::AddNode { node, index: None }
}

fn admit(actor: &str, key: &str, id: &str) -> Admission {
    let who = Principal::new("ws_1", actor);
    Gate::new()
        .submit(&who, vec![Op::new(key, add(id))])
        .unwrap()
        .admitted
        .remove(0)
}

type Reg = Rooms<Shared, MemoryActivityLog, Box<dyn FnMut(&str) -> Shared>>;

/// A registry of at most `max` rooms over one shared store; `made` counts the store handles it asked for.
fn registry(max: usize) -> (Reg, Rc<Cell<usize>>) {
    let store = Shared::default();
    let made = Rc::new(Cell::new(0));
    let counter = made.clone();
    let make: Box<dyn FnMut(&str) -> Shared> = Box::new(move |_| {
        counter.set(counter.get() + 1);
        store.clone()
    });
    (Rooms::new(max, make, MemoryActivityLog::new()), made)
}

fn put(reg: &mut Reg, file: &str, actor: &str, key: &str, id: &str) -> Result<u64, RegistryError> {
    reg.accept(file, admit(actor, key, id), 7)
        .map(|l| l.accepted.seq)
}

fn ids(reg: &Reg, file: &str) -> Vec<String> {
    reg.with_room(file, |r| r.nodes().iter().map(|n| n.id.clone()).collect())
        .unwrap()
}

#[test]
fn opening_the_same_file_twice_is_one_room() {
    let (mut reg, made) = registry(4);
    let first = reg.open("ws_1", "f1", 0).unwrap();
    assert!(!first.already_open);
    put(&mut reg, "f1", "a", "k1", "n1").unwrap();
    let second = reg.open("ws_1", "f1", 0).unwrap();
    assert!(second.already_open);
    assert_eq!(second.seq, 1);
    assert_eq!(ids(&reg, "f1"), ["n1"]);
    assert_eq!(reg.len(), 1);
    assert_eq!(made.get(), 1, "the second open must not ask for a store");
}

#[test]
fn a_file_open_in_one_workspace_is_refused_in_another() {
    let (mut reg, _) = registry(4);
    reg.open("ws_1", "f1", 0).unwrap();
    let err = reg.open("ws_2", "f1", 0).unwrap_err();
    assert_eq!(err, RegistryError::WorkspaceMismatch { file: "f1".into() });
    assert_eq!(reg.len(), 1);
}

#[test]
fn the_number_of_open_rooms_is_bounded_and_close_frees_a_slot() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 0).unwrap();
    reg.open("ws_1", "f2", 0).unwrap();
    assert_eq!(
        reg.open("ws_1", "f3", 0).unwrap_err(),
        RegistryError::Full { limit: 2 }
    );
    assert!(!reg.is_open("f3"));
    // A room that is already open is not a new one, so a full registry still answers.
    assert!(reg.open("ws_1", "f1", 0).unwrap().already_open);
    assert!(reg.close("f1"));
    reg.open("ws_1", "f3", 0).unwrap();
    assert_eq!(reg.len(), 2);
}

#[test]
fn a_workspace_at_its_room_limit_does_not_block_another_workspace() {
    let (reg, _) = registry(8);
    let mut reg = reg.with_workspace_limit(2);
    reg.open("ws_1", "f1", 0).unwrap();
    reg.open("ws_1", "f2", 0).unwrap();
    assert_eq!(
        reg.open("ws_1", "f3", 0).unwrap_err(),
        RegistryError::WorkspaceFull { limit: 2 }
    );
    assert!(!reg.is_open("f3"));
    // Already open is not a new room; another workspace has its own count; closing frees a slot.
    assert!(reg.open("ws_1", "f1", 0).unwrap().already_open);
    reg.open("ws_2", "g1", 0).unwrap();
    assert!(reg.close("f1"));
    reg.open("ws_1", "f3", 0).unwrap();
}

#[test]
fn closing_a_file_that_is_not_open_does_nothing() {
    let (mut reg, _) = registry(2);
    assert!(!reg.close("nope"));
    reg.open("ws_1", "f1", 0).unwrap();
    assert!(reg.close("f1"));
    assert!(!reg.close("f1"));
    assert_eq!(reg.len(), 0);
}

#[test]
fn a_write_to_a_file_that_is_not_open_is_refused_and_writes_nothing() {
    let (mut reg, _) = registry(2);
    let err = put(&mut reg, "f1", "a", "k1", "n1").unwrap_err();
    assert_eq!(err, RegistryError::NotOpen { file: "f1".into() });
    reg.open("ws_1", "f1", 0).unwrap();
    assert_eq!(ids(&reg, "f1"), Vec::<String>::new());
    assert!(reg.with_room("f2", |r| r.seq()).is_none());
}

#[test]
fn a_reopened_room_is_restored_from_the_store() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 0).unwrap();
    for (i, id) in ["n1", "n2", "n3"].iter().enumerate() {
        put(&mut reg, "f1", "a", &format!("k{i}"), id).unwrap();
    }
    assert!(reg.close("f1"));
    let reopened = reg.open("ws_1", "f1", 0).unwrap();
    assert_eq!((reopened.already_open, reopened.seq), (false, 3));
    assert_eq!(ids(&reg, "f1"), ["n1", "n2", "n3"]);
    // The log came back with the room: a retried key is a duplicate, a new key continues the numbers.
    let adm = admit("a", "k1", "n2");
    let dup = reg.accept("f1", adm, 9).unwrap().accepted;
    assert_eq!((dup.seq, dup.written), (2, false));
    assert_eq!(put(&mut reg, "f1", "a", "k9", "n9").unwrap(), 4);
}

#[test]
fn rooms_are_independent_and_each_has_its_own_order_and_activity() {
    let (mut reg, _) = registry(4);
    reg.open("ws_1", "f1", 0).unwrap();
    reg.open("ws_1", "f2", 0).unwrap();
    assert_eq!(put(&mut reg, "f1", "a", "k1", "n1").unwrap(), 1);
    assert_eq!(put(&mut reg, "f2", "a", "k1", "m1").unwrap(), 1);
    assert_eq!(put(&mut reg, "f1", "b", "k1", "n2").unwrap(), 2);
    assert_eq!(ids(&reg, "f1"), ["n1", "n2"]);
    assert_eq!(ids(&reg, "f2"), ["m1"]);
    let rows = |file: &str| {
        reg.activity()
            .entries_after(file, 0, usize::MAX)
            .unwrap()
            .into_iter()
            .map(|e| (e.seq, e.actor, e.node_id, e.at))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        rows("f1"),
        [
            (1, "a".into(), "n1".into(), 7),
            (2, "b".into(), "n2".into(), 7)
        ]
    );
    assert_eq!(rows("f2"), [(1, "a".into(), "m1".into(), 7)]);
}

#[test]
fn a_retry_or_a_refusal_through_the_registry_records_nothing() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 0).unwrap();
    put(&mut reg, "f1", "a", "k1", "n1").unwrap();
    put(&mut reg, "f1", "a", "k1", "n1").unwrap();
    let err = put(&mut reg, "f1", "a", "k2", "n1").unwrap_err();
    assert!(
        matches!(err, RegistryError::Room(RoomError::Refused(_))),
        "{err:?}"
    );
    assert_eq!(reg.activity().entries_after("f1", 0, 10).unwrap().len(), 1);
    assert_eq!(reg.activity_failures(), 0);
}

#[test]
fn a_permit_of_another_workspace_is_refused_on_an_open_room() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 0).unwrap();
    let other = Principal::new("ws_2", "a");
    let a = Gate::new()
        .submit(&other, vec![Op::new("k1", add("n1"))])
        .unwrap()
        .admitted
        .remove(0);
    let err = reg.accept("f1", a, 1).unwrap_err();
    match err {
        RegistryError::Room(RoomError::Refused(r)) => {
            assert_eq!(r.code(), "invalid");
            assert!(r.to_string().contains("workspace"), "{r}");
        }
        other => panic!("{other:?}"),
    }
    assert!(ids(&reg, "f1").is_empty());
}

#[test]
fn a_registry_that_holds_no_room_refuses_every_open_without_asking_for_a_store() {
    let (mut reg, made) = registry(0);
    assert_eq!(
        reg.open("ws_1", "f1", 0).unwrap_err(),
        RegistryError::Full { limit: 0 }
    );
    assert_eq!(made.get(), 0);
    assert_eq!(reg.len(), 0);
}

#[test]
fn an_open_that_fails_leaves_no_half_registered_room() {
    // The first store the factory hands out has a log that does not replay; the second is good.
    let mut bad = MemoryStore::new();
    let o = admit("a", "k1", "n1");
    bad.append(
        "f1",
        NewOp {
            actor: "a".into(),
            key: "k1".into(),
            command: o.op().command.clone(),
        },
    )
    .unwrap();
    // A second op that the document core refuses (the same node id again) makes the log corrupt.
    let o2 = admit("a", "k2", "n1");
    bad.append(
        "f1",
        NewOp {
            actor: "a".into(),
            key: "k2".into(),
            command: o2.op().command.clone(),
        },
    )
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    let bad = Shared(Rc::new(RefCell::new(bad)));
    let make: Box<dyn FnMut(&str) -> Shared> = Box::new(move |_| {
        counter.set(counter.get() + 1);
        if counter.get() == 1 {
            bad.clone()
        } else {
            Shared::default()
        }
    });
    let mut reg: Reg = Rooms::new(2, make, MemoryActivityLog::new());
    let err = reg.open("ws_1", "f1", 0).unwrap_err();
    assert!(
        matches!(err, RegistryError::Room(RoomError::Corrupt { .. })),
        "{err:?}"
    );
    assert_eq!(reg.len(), 0);
    reg.open("ws_1", "f1", 0)
        .expect("a retry can open with a good store");
    assert_eq!(reg.len(), 1);
}

// Idle close. The clock is the caller's: `open` and `accept` take a value, `close_idle` takes `now`.
// A room is touched by `open` (also of an open file) and by an accepted write, not by `with_room`.

fn put_at(reg: &mut Reg, file: &str, key: &str, id: &str, at: u64) {
    reg.accept(file, admit("a", key, id), at).unwrap();
}

#[test]
fn an_idle_room_closes_frees_its_slot_and_is_restored_on_open() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 10).unwrap();
    put_at(&mut reg, "f1", "k1", "n1", 20);
    reg.open("ws_1", "f2", 50).unwrap();
    assert_eq!(reg.close_idle(120, 100), ["f1"].map(String::from));
    assert_eq!(
        (reg.is_open("f1"), reg.is_open("f2"), reg.len()),
        (false, true, 1)
    );
    let back = reg.open("ws_1", "f1", 121).unwrap();
    assert_eq!((back.already_open, back.seq), (false, 1));
    assert_eq!(ids(&reg, "f1"), ["n1"]);
}

#[test]
fn a_room_touched_since_stays_open() {
    let (mut reg, _) = registry(4);
    reg.open("ws_1", "f1", 0).unwrap();
    reg.open("ws_1", "f2", 0).unwrap();
    reg.open("ws_1", "f3", 0).unwrap();
    put_at(&mut reg, "f1", "k1", "n1", 90); // accepted write
    reg.open("ws_1", "f2", 95).unwrap(); // open of an open file
    // f3 is only read: a read does not touch.
    reg.with_room("f3", |r| r.seq()).unwrap();
    assert_eq!(reg.close_idle(100, 50), ["f3"].map(String::from));
    assert!(reg.is_open("f1") && reg.is_open("f2"));
}

#[test]
fn a_refused_write_is_not_a_touch() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 0).unwrap();
    put_at(&mut reg, "f1", "k1", "n1", 10);
    // A refused op (same node id again) is not an accepted write: no touch.
    assert!(reg.accept("f1", admit("a", "k2", "n1"), 500).is_err());
    assert_eq!(reg.close_idle(500, 100), ["f1"].map(String::from));
}

#[test]
fn a_retried_key_is_not_a_touch() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 0).unwrap();
    put_at(&mut reg, "f1", "k1", "n1", 10);
    // The same key again, later: a duplicate (nothing written), so the last touch stays at 10.
    put_at(&mut reg, "f1", "k1", "n1", 500);
    assert_eq!(reg.close_idle(500, 100), ["f1"].map(String::from));
}

#[test]
fn the_ids_come_back_sorted_whatever_the_open_order() {
    let (mut reg, _) = registry(4);
    for f in ["f3", "f1", "f2"] {
        reg.open("ws_1", f, 0).unwrap();
    }
    assert_eq!(reg.close_idle(10, 10), ["f1", "f2", "f3"].map(String::from));
    assert!(reg.is_empty());
}

#[test]
fn idle_for_zero_closes_everything_and_a_huge_one_nothing() {
    let (mut reg, _) = registry(4);
    reg.open("ws_1", "f1", 5).unwrap();
    reg.open("ws_1", "f2", 5).unwrap();
    assert!(reg.close_idle(1000, u64::MAX).is_empty());
    assert_eq!(reg.len(), 2);
    assert_eq!(reg.close_idle(5, 0), ["f1", "f2"].map(String::from));
    assert!(reg.is_empty());
}

#[test]
fn a_clock_that_goes_backwards_never_closes_a_room() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 100).unwrap();
    assert!(reg.close_idle(50, 0).is_empty());
    // A touch from the past does not move the last touch back either.
    put_at(&mut reg, "f1", "k1", "n1", 10);
    assert!(reg.close_idle(105, 10).is_empty());
    assert_eq!(reg.close_idle(110, 10), ["f1"].map(String::from));
}

#[test]
fn a_full_registry_that_closes_idle_rooms_can_open_a_new_file() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1", 0).unwrap();
    reg.open("ws_1", "f2", 90).unwrap();
    assert_eq!(
        reg.open("ws_1", "f3", 100).unwrap_err(),
        RegistryError::Full { limit: 2 }
    );
    assert_eq!(reg.close_idle(100, 50), ["f1"].map(String::from));
    reg.open("ws_1", "f3", 100).unwrap();
    assert_eq!(
        (reg.len(), reg.is_open("f2"), reg.is_open("f3")),
        (2, true, true)
    );
}
