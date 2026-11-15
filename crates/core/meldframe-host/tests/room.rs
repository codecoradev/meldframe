//! The room: one total order per file, one write entry (`accept`), the dependency rule (issue 96).

mod common;

use common::{add, admit, node, op, refusal};
use meldframe_host::{Gate, MemoryStore, Op, Principal, Room, Store};
use meldframe_protocol::{Command, MAX_OP_DEPS};

fn room() -> Room<MemoryStore> {
    Room::open("ws_1", "file_1", MemoryStore::new()).unwrap()
}

fn put(room: &mut Room<MemoryStore>, actor: &str, o: Op) -> Result<u64, meldframe_host::RoomError> {
    let (o, p) = admit(actor, o);
    room.accept(o, p).map(|a| a.seq)
}

fn ids(room: &Room<MemoryStore>) -> Vec<String> {
    room.nodes().iter().map(|n| n.id.clone()).collect()
}

#[test]
fn an_admitted_op_is_applied_and_numbered() {
    let mut r = room();
    assert_eq!(put(&mut r, "a", op("k1", add("n1"))).unwrap(), 1);
    assert_eq!(put(&mut r, "a", op("k2", add("n2"))).unwrap(), 2);
    assert_eq!(ids(&r), ["n1", "n2"]);
    assert_eq!(r.seq(), 2);
}

#[test]
fn ops_of_two_actors_have_one_total_order() {
    let mut r = room();
    let order = [("a", "k1", "n1"), ("b", "k1", "n2"), ("a", "k2", "n3")];
    for (actor, key, id) in order {
        put(&mut r, actor, op(key, add(id))).unwrap();
    }
    let log = r.store().ops_after("file_1", 0, usize::MAX).unwrap();
    let rows: Vec<(u64, &str, &str)> = log
        .iter()
        .map(|o| (o.seq, o.actor.as_str(), o.key.as_str()))
        .collect();
    assert_eq!(rows, [(1, "a", "k1"), (2, "b", "k1"), (3, "a", "k2")]);
}

#[test]
fn a_retried_key_gets_the_same_number_and_changes_nothing() {
    let mut r = room();
    put(&mut r, "a", op("k1", add("n1"))).unwrap();
    put(&mut r, "a", op("k2", add("n2"))).unwrap();
    let (o, p) = admit("a", op("k1", add("n1")));
    let again = r.accept(o, p).unwrap();
    assert_eq!((again.seq, again.written), (1, false));
    assert_eq!(r.seq(), 2);
    assert_eq!(ids(&r), ["n1", "n2"]);
}

#[test]
fn a_refused_command_writes_nothing_and_its_key_can_be_used_again() {
    let mut r = room();
    // The parent does not exist.
    let bad = Command::AddNode {
        node: node("c", "rect", Some("missing")),
        index: None,
    };
    let err = refusal(put(&mut r, "a", op("k1", bad)).unwrap_err());
    assert_eq!(err.code(), "E_PARENT_NOT_FOUND");
    assert_eq!(r.seq(), 0);
    assert!(r.nodes().is_empty());
    assert_eq!(put(&mut r, "a", op("k1", add("n1"))).unwrap(), 1);
}

#[test]
fn a_dependency_must_have_been_applied() {
    let mut r = room();
    put(&mut r, "a", op("k1", add("n1"))).unwrap();
    let ok = op("k2", add("n2")).with_deps(["k1"]);
    assert_eq!(put(&mut r, "a", ok).unwrap(), 2);
    let err = refusal(put(&mut r, "a", op("k3", add("n3")).with_deps(["nope"])).unwrap_err());
    assert_eq!(err.code(), "dependency_rejected");
    assert_eq!(r.seq(), 2);
}

#[test]
fn a_dependency_on_another_actors_key_is_refused() {
    let mut r = room();
    put(&mut r, "a", op("k1", add("n1"))).unwrap();
    let dep = op("k1b", add("n2")).with_deps(["k1"]);
    let err = refusal(put(&mut r, "b", dep).unwrap_err());
    assert_eq!(err.code(), "dependency_rejected");
    assert_eq!(r.seq(), 1);
}

#[test]
fn a_forward_dependency_is_refused_until_the_dependency_is_applied() {
    let mut r = room();
    let late = op("k1", add("n1")).with_deps(["k2"]);
    assert!(put(&mut r, "a", late.clone()).is_err());
    put(&mut r, "a", op("k2", add("n2"))).unwrap();
    assert_eq!(put(&mut r, "a", late).unwrap(), 2);
}

#[test]
fn the_most_dependencies_are_accepted_when_all_were_applied() {
    let mut r = room();
    let keys: Vec<String> = (0..MAX_OP_DEPS).map(|i| format!("d{i}")).collect();
    for k in &keys {
        put(&mut r, "a", op(k, add(k))).unwrap();
    }
    let big = op("last", add("last")).with_deps(keys);
    assert_eq!(put(&mut r, "a", big).unwrap(), MAX_OP_DEPS as u64 + 1);
}

#[test]
fn if_seq_refuses_an_edit_of_a_node_changed_since() {
    let mut r = room();
    put(&mut r, "a", op("k1", add("n1"))).unwrap();
    let upd = |key: &str| {
        let patch = meldframe_protocol::NodePatch {
            x: Some(5.0),
            ..Default::default()
        };
        op(
            key,
            Command::UpdateNode {
                id: "n1".into(),
                patch,
            },
        )
    };
    put(&mut r, "b", upd("k2")).unwrap();
    let err = refusal(put(&mut r, "a", upd("k3").with_if_seq(1)).unwrap_err());
    assert_eq!(err.code(), "conflict");
    assert_eq!(r.seq(), 2);
    assert_eq!(put(&mut r, "a", upd("k3").with_if_seq(2)).unwrap(), 3);
}

#[test]
fn a_permit_for_another_op_or_workspace_is_refused() {
    let mut r = room();
    let (_, permit) = admit("a", op("k1", add("n1")));
    let err = refusal(r.accept(op("k2", add("n2")), permit).unwrap_err());
    assert_eq!(err.code(), "invalid");
    let other = Principal::new("ws_2", "a");
    let adm = Gate::new()
        .submit(&other, vec![op("k1", add("n1"))])
        .unwrap()
        .admitted
        .remove(0);
    let err = refusal(r.accept(adm.op, adm.permit).unwrap_err());
    assert_eq!(err.code(), "invalid");
    assert!(err.to_string().contains("workspace"), "{err}");
    assert_eq!(r.seq(), 0);
}

#[test]
fn a_key_reused_for_a_different_command_is_refused_not_reported_as_done() {
    let mut r = room();
    put(&mut r, "a", op("k1", add("n1"))).unwrap();
    let err = refusal(put(&mut r, "a", op("k1", add("other"))).unwrap_err());
    assert_eq!(err.code(), "invalid");
    assert_eq!(ids(&r), ["n1"]);
    assert_eq!(r.seq(), 1);
}

#[test]
fn a_permit_does_not_cover_a_stronger_command_under_the_same_key() {
    // The gate decided the policy for a Change; the same key with a Delete must not ride on it.
    let mut r = room();
    put(&mut r, "a", op("k1", add("n1"))).unwrap();
    let (_, permit) = admit("a", op("k2", add("n2")));
    let err = refusal(
        r.accept(op("k2", common::remove("n1")), permit)
            .unwrap_err(),
    );
    assert_eq!(err.code(), "invalid");
    assert_eq!(ids(&r), ["n1"]);
}
