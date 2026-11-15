//! The room registry: open is idempotent, close frees a slot, the number of open rooms is bounded, a room
//! is restored from the store on open, and every write goes through the registry.

use meldframe_host::{
    Appended, Gate, MemoryStore, NewOp, Op, Permit, Principal, RoomError, Snapshot, Store,
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

fn admit(actor: &str, key: &str, id: &str) -> (Op, Permit) {
    let who = Principal::new("ws_1", actor);
    let a = Gate::new()
        .submit(&who, vec![Op::new(key, add(id))])
        .unwrap()
        .admitted
        .remove(0);
    (a.op, a.permit)
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
    let (o, p) = admit(actor, key, id);
    reg.accept(file, o, p, 7).map(|l| l.accepted.seq)
}

fn ids(reg: &Reg, file: &str) -> Vec<String> {
    reg.with_room(file, |r| r.nodes().iter().map(|n| n.id.clone()).collect())
        .unwrap()
}

#[test]
fn opening_the_same_file_twice_is_one_room() {
    let (mut reg, made) = registry(4);
    let first = reg.open("ws_1", "f1").unwrap();
    assert!(!first.already_open);
    put(&mut reg, "f1", "a", "k1", "n1").unwrap();
    let second = reg.open("ws_1", "f1").unwrap();
    assert!(second.already_open);
    assert_eq!(second.seq, 1);
    assert_eq!(ids(&reg, "f1"), ["n1"]);
    assert_eq!(reg.len(), 1);
    assert_eq!(made.get(), 1, "the second open must not ask for a store");
}

#[test]
fn a_file_open_in_one_workspace_is_refused_in_another() {
    let (mut reg, _) = registry(4);
    reg.open("ws_1", "f1").unwrap();
    let err = reg.open("ws_2", "f1").unwrap_err();
    assert_eq!(err, RegistryError::WorkspaceMismatch { file: "f1".into() });
    assert_eq!(reg.len(), 1);
}

#[test]
fn the_number_of_open_rooms_is_bounded_and_close_frees_a_slot() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1").unwrap();
    reg.open("ws_1", "f2").unwrap();
    assert_eq!(
        reg.open("ws_1", "f3").unwrap_err(),
        RegistryError::Full { limit: 2 }
    );
    assert!(!reg.is_open("f3"));
    // A room that is already open is not a new one, so a full registry still answers.
    assert!(reg.open("ws_1", "f1").unwrap().already_open);
    assert!(reg.close("f1"));
    reg.open("ws_1", "f3").unwrap();
    assert_eq!(reg.len(), 2);
}

#[test]
fn closing_a_file_that_is_not_open_does_nothing() {
    let (mut reg, _) = registry(2);
    assert!(!reg.close("nope"));
    reg.open("ws_1", "f1").unwrap();
    assert!(reg.close("f1"));
    assert!(!reg.close("f1"));
    assert_eq!(reg.len(), 0);
}

#[test]
fn a_write_to_a_file_that_is_not_open_is_refused_and_writes_nothing() {
    let (mut reg, _) = registry(2);
    let err = put(&mut reg, "f1", "a", "k1", "n1").unwrap_err();
    assert_eq!(err, RegistryError::NotOpen { file: "f1".into() });
    reg.open("ws_1", "f1").unwrap();
    assert_eq!(ids(&reg, "f1"), Vec::<String>::new());
    assert!(reg.with_room("f2", |r| r.seq()).is_none());
}

#[test]
fn a_reopened_room_is_restored_from_the_store() {
    let (mut reg, _) = registry(2);
    reg.open("ws_1", "f1").unwrap();
    for (i, id) in ["n1", "n2", "n3"].iter().enumerate() {
        put(&mut reg, "f1", "a", &format!("k{i}"), id).unwrap();
    }
    assert!(reg.close("f1"));
    let reopened = reg.open("ws_1", "f1").unwrap();
    assert_eq!((reopened.already_open, reopened.seq), (false, 3));
    assert_eq!(ids(&reg, "f1"), ["n1", "n2", "n3"]);
    // The log came back with the room: a retried key is a duplicate, a new key continues the numbers.
    let (o, p) = admit("a", "k1", "n2");
    let dup = reg.accept("f1", o, p, 9).unwrap().accepted;
    assert_eq!((dup.seq, dup.written), (2, false));
    assert_eq!(put(&mut reg, "f1", "a", "k9", "n9").unwrap(), 4);
}

#[test]
fn rooms_are_independent_and_each_has_its_own_order_and_activity() {
    let (mut reg, _) = registry(4);
    reg.open("ws_1", "f1").unwrap();
    reg.open("ws_1", "f2").unwrap();
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
    reg.open("ws_1", "f1").unwrap();
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
    reg.open("ws_1", "f1").unwrap();
    let other = Principal::new("ws_2", "a");
    let a = Gate::new()
        .submit(&other, vec![Op::new("k1", add("n1"))])
        .unwrap()
        .admitted
        .remove(0);
    let err = reg.accept("f1", a.op, a.permit, 1).unwrap_err();
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
        reg.open("ws_1", "f1").unwrap_err(),
        RegistryError::Full { limit: 0 }
    );
    assert_eq!(made.get(), 0);
    assert_eq!(reg.len(), 0);
}

#[test]
fn an_open_that_fails_leaves_no_half_registered_room() {
    // The first store the factory hands out has a log that does not replay; the second is good.
    let mut bad = MemoryStore::new();
    let (o, _) = admit("a", "k1", "n1");
    bad.append(
        "f1",
        NewOp {
            actor: "a".into(),
            key: "k1".into(),
            command: o.command,
        },
    )
    .unwrap();
    // A second op that the document core refuses (the same node id again) makes the log corrupt.
    let (o2, _) = admit("a", "k2", "n1");
    bad.append(
        "f1",
        NewOp {
            actor: "a".into(),
            key: "k2".into(),
            command: o2.command,
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
    let err = reg.open("ws_1", "f1").unwrap_err();
    assert!(
        matches!(err, RegistryError::Room(RoomError::Corrupt { .. })),
        "{err:?}"
    );
    assert_eq!(reg.len(), 0);
    reg.open("ws_1", "f1")
        .expect("a retry can open with a good store");
    assert_eq!(reg.len(), 1);
}
