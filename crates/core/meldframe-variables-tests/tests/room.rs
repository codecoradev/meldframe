//! E3.3: variable commands go through the room like node commands: one total order, `ifSeq` on a variable, a
//! restart that replays them, and a snapshot that carries them.

mod common;

use common::{add, appearance, bg, cmd};
use meldframe_host::{Admission, Gate, MemoryStore, Op, Principal, Refusal, Room, RoomError};
use meldframe_protocol::{Bind, Command};
use serde_json::json;

/// The admission the gate gives an op of actor `a` in workspace `ws_1`.
fn admit(op: Op) -> Admission {
    let who = Principal::new("ws_1", "a");
    Gate::new()
        .submit(&who, vec![op])
        .unwrap()
        .admitted
        .remove(0)
}

fn refusal(e: RoomError) -> Refusal {
    match e {
        RoomError::Refused(r) => r,
        other => panic!("expected a refusal, got {other}"),
    }
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
    r.accept(admit(o)).map(|a| a.seq).map_err(refusal)
}

fn setup() -> Vec<Command> {
    vec![appearance(), bg()]
}

#[test]
fn a_room_applies_variable_commands_and_refuses_a_bad_binding() {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    for (i, c) in setup().into_iter().enumerate() {
        put(&mut r, &format!("k{i}"), c, None).unwrap();
    }
    put(&mut r, "k2", add("n1", "rect", None), None).unwrap();
    let bind =
        |v: &str| cmd(json!({"type": "updateNode", "id": "n1", "patch": {"fill": {"var": v}}}));
    assert!(
        matches!(put(&mut r, "k3", bind("gone"), None), Err(Refusal::Apply(e)) if e.code() == "E_VARIABLE_NOT_FOUND")
    );
    put(&mut r, "k4", bind("bg"), None).unwrap();
    assert_eq!(r.nodes()[0].fill, Bind::var("bg"));
    assert_eq!(r.vars().variables.len(), 1);
}

#[test]
fn if_seq_protects_a_value_another_op_changed() {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    for (i, c) in setup().into_iter().enumerate() {
        put(&mut r, &format!("k{i}"), c, None).unwrap();
    }
    let seen = r.seq();
    let set =
        |v: &str| cmd(json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": v}));
    put(&mut r, "k3", set("#111111"), Some(seen)).unwrap();
    // A second writer that still works from `seen` is refused, and the refusal names the variable.
    match put(&mut r, "k4", set("#222222"), Some(seen)) {
        Err(Refusal::Conflict { node_id }) => assert_eq!(node_id, "$var:bg"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_restart_replays_variable_commands_and_a_snapshot_keeps_them() {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new())
        .unwrap()
        .with_snapshot_every(2);
    for (i, c) in setup().into_iter().enumerate() {
        put(&mut r, &format!("k{i}"), c, None).unwrap();
    }
    put(
        &mut r,
        "k2",
        cmd(json!({"type": "setVariableValue", "id": "bg", "mode": "light", "value": "#eeeeee"})),
        None,
    )
    .unwrap();
    let want = r.vars().clone();
    // Snapshot at 2 (both variable commands), the third op is replayed from the log.
    let again = Room::open("ws_1", "file_1", r.into_store()).unwrap();
    assert_eq!(again.vars(), &want);
}
