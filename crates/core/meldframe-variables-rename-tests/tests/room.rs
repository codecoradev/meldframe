//! A rename through the room (D8): one total order, `ifSeq` on the entity, a restart that replays it, a snapshot that
//! carries it, the risk tier and the activity kind of the other variable commands.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-variables-tests/tests/common/mod.rs"]
mod common;

use common::{appearance, bg, cmd};
use meldframe_host::{Admission, Gate, MemoryStore, Op, Principal, Refusal, Room, RoomError};
use meldframe_protocol::Command;
use meldframe_rooms::{ActivityEntry, ActivityKind};
use serde_json::json;

fn admit(op: Op) -> Admission {
    let who = Principal::new("ws_1", "a");
    Gate::new()
        .submit(&who, vec![op])
        .unwrap()
        .admitted
        .remove(0)
}

fn put(
    r: &mut Room<MemoryStore>,
    key: &str,
    c: Command,
    if_seq: Option<u64>,
) -> Result<u64, Refusal> {
    let mut o = Op::new(key, c);
    if let Some(s) = if_seq {
        o = o.with_if_seq(s);
    }
    match r.accept(admit(o)) {
        Ok(a) => Ok(a.seq),
        Err(RoomError::Refused(r)) => Err(r),
        Err(e) => panic!("{e}"),
    }
}

fn room() -> Room<MemoryStore> {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    put(&mut r, "k0", appearance(), None).unwrap();
    put(&mut r, "k1", bg(), None).unwrap();
    r
}

fn rename_variable(id: &str, name: &str) -> Command {
    cmd(json!({"type": "renameVariable", "id": id, "name": name}))
}

fn rename_collection(id: &str, name: &str) -> Command {
    cmd(json!({"type": "renameCollection", "id": id, "name": name}))
}

fn rename_mode(collection: &str, mode: &str, name: &str) -> Command {
    cmd(json!({"type": "renameMode", "collection": collection, "mode": mode, "name": name}))
}

#[test]
fn a_room_applies_a_rename_and_refuses_a_bad_one_with_its_own_code() {
    let mut r = room();
    put(&mut r, "k2", rename_variable("bg", "color/surface"), None).unwrap();
    put(&mut r, "k3", rename_mode("t", "dark", "Night"), None).unwrap();
    put(&mut r, "k4", rename_collection("t", "theme"), None).unwrap();
    let c = r.vars().collection("t").unwrap();
    assert_eq!(
        (c.name.as_str(), c.modes[1].name.as_str()),
        ("theme", "Night")
    );
    assert_eq!(r.vars().variable("bg").unwrap().name, "color/surface");
    for (key, c, code) in [
        (
            "k5",
            rename_variable("gone", "color/x"),
            "E_VARIABLE_NOT_FOUND",
        ),
        ("k6", rename_variable("bg", "Not A Name"), "E_INVALID_NAME"),
        ("k7", rename_mode("t", "gone", "x"), "E_MODE_NOT_FOUND"),
        (
            "k8",
            rename_collection("gone", "x"),
            "E_COLLECTION_NOT_FOUND",
        ),
    ] {
        match put(&mut r, key, c, None) {
            Err(Refusal::Apply(e)) => assert_eq!(e.code(), code),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn if_seq_protects_a_name_another_op_changed() {
    let mut r = room();
    let seen = r.seq();
    put(&mut r, "k2", rename_variable("bg", "color/a"), Some(seen)).unwrap();
    match put(&mut r, "k3", rename_variable("bg", "color/b"), Some(seen)) {
        Err(Refusal::Conflict { node_id }) => assert_eq!(node_id, "$var:bg"),
        other => panic!("{other:?}"),
    }
    // The rename of a variable did not touch the collection; a mode and a collection rename both write to it.
    put(&mut r, "k4", rename_mode("t", "dark", "Night"), Some(seen)).unwrap();
    match put(&mut r, "k5", rename_collection("t", "theme"), Some(seen)) {
        Err(Refusal::Conflict { node_id }) => assert_eq!(node_id, "$col:t"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_restart_replays_a_rename_and_a_snapshot_keeps_it() {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new())
        .unwrap()
        .with_snapshot_every(3);
    put(&mut r, "k0", appearance(), None).unwrap();
    put(&mut r, "k1", bg(), None).unwrap();
    put(&mut r, "k2", rename_variable("bg", "color/surface"), None).unwrap();
    put(&mut r, "k3", rename_mode("t", "dark", "Night"), None).unwrap();
    put(&mut r, "k4", rename_collection("t", "theme"), None).unwrap();
    let want = r.vars().clone();
    assert_eq!(want.variable("bg").unwrap().name, "color/surface");
    assert_eq!(want.collection("t").unwrap().name, "theme");
    // The snapshot at 3 holds the first rename; the other two are replayed from the log.
    let again = Room::open("ws_1", "file_1", r.into_store()).unwrap();
    assert_eq!(again.vars(), &want);
}

#[test]
fn a_rename_is_change_tier_and_the_activity_kind_is_variable() {
    use meldframe_module::RiskTier;
    for (c, id) in [
        (rename_variable("bg", "color/x"), "bg"),
        (rename_collection("t", "x"), "t"),
        (rename_mode("t", "dark", "x"), "t"),
    ] {
        assert_eq!(Op::new("k", c.clone()).tier(), RiskTier::Change);
        assert_eq!(
            ActivityEntry::describe(&c),
            (ActivityKind::Variable, id.to_string())
        );
    }
}
