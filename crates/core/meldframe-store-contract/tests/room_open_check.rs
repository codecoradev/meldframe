//! A stored snapshot is checked when the room opens (`check_file`, after the old-file migration). A file
//! that fails is refused with a typed error and its bytes stay as they are.

use meldframe_host::{
    Admission, Gate, MemoryStore, Op, Principal, Room, RoomError, Snapshot, Store,
};
use meldframe_protocol::Command;
use meldframe_protocol::Node;
use serde_json::json;

fn admit(op: Op) -> Admission {
    let who = Principal::new("ws_1", "a");
    Gate::new()
        .submit(&who, vec![op])
        .unwrap()
        .admitted
        .remove(0)
}

fn n(v: serde_json::Value) -> Node {
    serde_json::from_value(v).unwrap()
}

/// A store whose log has one op (so a snapshot at seq 1 is allowed) and that holds `nodes` as the snapshot.
fn store_with(nodes: Vec<Node>) -> MemoryStore {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    let node = n(json!({"id": "n0", "type": "frame"}));
    r.accept(admit(Op::new("k0", Command::AddNode { node, index: None })))
        .unwrap();
    let mut store = r.into_store();
    store
        .save_snapshot("file_1", Snapshot { seq: 1, nodes })
        .unwrap();
    store
}

fn code_of(nodes: Vec<Node>) -> &'static str {
    match Room::open("ws_1", "file_1", store_with(nodes)) {
        Err(RoomError::BadSnapshot { seq, error }) => {
            assert_eq!(seq, 1);
            error.code()
        }
        other => panic!("expected BadSnapshot, got {other:?}"),
    }
}

fn frame(id: &str, parent: Option<&str>, pos: &str) -> serde_json::Value {
    json!({"id": id, "type": "frame", "parentId": parent, "pos": pos})
}

#[test]
fn a_good_snapshot_opens_and_an_empty_file_still_opens() {
    let good = vec![n(frame("a", None, "a")), n(frame("b", Some("a"), "a"))];
    let room = Room::open("ws_1", "file_1", store_with(good)).unwrap();
    assert_eq!(room.nodes().len(), 2);
    assert!(Room::open("ws_1", "file_1", MemoryStore::new()).is_ok());
}

#[test]
fn a_snapshot_with_a_cycle_a_duplicate_id_or_a_missing_parent_is_refused() {
    let cycle = vec![n(frame("a", Some("b"), "a")), n(frame("b", Some("a"), "a"))];
    assert_eq!(code_of(cycle), "E_CYCLE");
    let dup = vec![n(frame("a", None, "a")), n(frame("a", None, "b"))];
    assert_eq!(code_of(dup), "E_DUPLICATE_ID");
    let orphan = vec![n(frame("a", Some("ghost"), "a"))];
    assert_eq!(code_of(orphan), "E_PARENT_NOT_FOUND");
}

#[test]
fn an_old_snapshot_without_positions_is_migrated_not_refused() {
    let old = vec![
        n(json!({"id": "a", "type": "frame", "parentId": null})),
        n(json!({"id": "b", "type": "frame", "parentId": null})),
        n(json!({"id": "c", "type": "frame", "parentId": "a"})),
    ];
    let room = Room::open("ws_1", "file_1", store_with(old)).unwrap();
    assert!(room.nodes().iter().all(|x| !x.pos.is_empty()));
    let a = room.nodes().iter().find(|x| x.id == "a").unwrap();
    let b = room.nodes().iter().find(|x| x.id == "b").unwrap();
    assert!(a.pos < b.pos, "list order is kept");
}

#[test]
fn a_snapshot_with_equal_positions_is_migrated_like_the_loader_does() {
    let same = vec![n(frame("a", None, "a")), n(frame("b", None, "a"))];
    let room = Room::open("ws_1", "file_1", store_with(same)).unwrap();
    assert_ne!(room.nodes()[0].pos, room.nodes()[1].pos);
}
