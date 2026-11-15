//! The op dependency rule of the room (issue 96; written down in `docs/architecture.md`, section 7a).
//!
//! Every clause is pinned at the public seam: a `Gate` batch, then `Room::accept` per op in order.
//! These tests pin behaviour that already held; none of them needed a code change.

use meldframe_host::{
    Admission, Gate, MemoryStore, Op, Principal, Refusal, Room, RoomError, Store,
};
use meldframe_protocol::{Command, MAX_OP_DEPS};

type R = Room<MemoryStore>;

fn room() -> R {
    Room::open("ws_1", "file_1", MemoryStore::new()).unwrap()
}

fn add(id: &str) -> Command {
    let node = serde_json::from_value(serde_json::json!({"id": id, "type": "frame"})).unwrap();
    Command::AddNode { node, index: None }
}

/// An add whose parent does not exist: the document core refuses it.
fn bad(id: &str) -> Command {
    let node = serde_json::from_value(
        serde_json::json!({"id": id, "type": "frame", "parentId": "missing"}),
    )
    .unwrap();
    Command::AddNode { node, index: None }
}

fn op(key: &str, id: &str, deps: &[&str]) -> Op {
    Op::new(key, add(id)).with_deps(deps.iter().copied())
}

/// The gate, then the room, for one op.
fn put(r: &mut R, actor: &str, o: Op) -> Result<(u64, bool), Refusal> {
    let a = Gate::new()
        .submit(&Principal::new("ws_1", actor), vec![o])?
        .admitted
        .remove(0);
    send(r, a)
}

fn send(r: &mut R, a: Admission) -> Result<(u64, bool), Refusal> {
    match r.accept(a) {
        Ok(d) => Ok((d.seq, d.written)),
        Err(RoomError::Refused(x)) => Err(x),
        Err(other) => panic!("the room failed: {other}"),
    }
}

/// A batch: one gate decision, then every op to the room in the batch's order. Stops at nothing: the
/// result of each op is returned.
fn batch(r: &mut R, actor: &str, ops: Vec<Op>) -> Vec<Result<(u64, bool), Refusal>> {
    let report = Gate::new()
        .submit(&Principal::new("ws_1", actor), ops)
        .unwrap();
    report.admitted.into_iter().map(|a| send(r, a)).collect()
}

fn code(e: Result<(u64, bool), Refusal>) -> &'static str {
    e.expect_err("the op was accepted").code()
}

fn ids(r: &R) -> Vec<String> {
    r.nodes().iter().map(|n| n.id.clone()).collect()
}

fn restart(r: R) -> R {
    Room::open("ws_1", "file_1", r.into_store()).unwrap()
}

// Same actor only.

#[test]
fn a_dependency_on_a_key_of_another_actor_is_refused() {
    let mut r = room();
    put(&mut r, "a", op("k1", "n1", &[])).unwrap();
    assert_eq!(
        code(put(&mut r, "b", op("k2", "n2", &["k1"]))),
        "dependency_rejected"
    );
    assert_eq!(r.seq(), 1);
    // The same key is a different key for b: once b applied its own k1, the dependency holds.
    put(&mut r, "b", op("k1", "n3", &[])).unwrap();
    assert_eq!(
        put(&mut r, "b", op("k2", "n2", &["k1"])).unwrap(),
        (3, true)
    );
}

#[test]
fn an_unknown_key_is_refused_and_names_the_key() {
    let mut r = room();
    let err = put(&mut r, "a", op("k1", "n1", &["nope"])).unwrap_err();
    assert_eq!(err, Refusal::DependencyRejected { key: "nope".into() });
    assert_eq!((r.seq(), ids(&r).len()), (0, 0));
}

#[test]
fn every_dependency_must_be_applied_not_just_one() {
    let mut r = room();
    put(&mut r, "a", op("k1", "n1", &[])).unwrap();
    let err = put(&mut r, "a", op("k2", "n2", &["k1", "nope"])).unwrap_err();
    assert_eq!(err, Refusal::DependencyRejected { key: "nope".into() });
    assert_eq!(r.seq(), 1);
}

#[test]
fn the_refusal_names_the_first_missing_key_whatever_the_order() {
    let mut r = room();
    put(&mut r, "a", op("k1", "n1", &[])).unwrap();
    let err = put(&mut r, "a", op("k2", "n2", &["m1", "m2"])).unwrap_err();
    assert_eq!(err, Refusal::DependencyRejected { key: "m1".into() });
    // An applied key after a missing one does not hide it.
    let err = put(&mut r, "a", op("k3", "n3", &["nope", "k1"])).unwrap_err();
    assert_eq!(err, Refusal::DependencyRejected { key: "nope".into() });
}

#[test]
fn an_empty_or_a_repeated_dependency_key_is_invalid_at_the_gate() {
    let mut r = room();
    put(&mut r, "a", op("k1", "n1", &[])).unwrap();
    assert_eq!(code(put(&mut r, "a", op("k2", "n2", &[""]))), "invalid");
    assert_eq!(
        code(put(&mut r, "a", op("k3", "n3", &["k1", "k1"]))),
        "invalid"
    );
    assert_eq!(r.seq(), 1);
}

// Forward, self and cyclic dependencies.

#[test]
fn a_forward_dependency_is_refused_until_its_dependency_is_applied() {
    let mut r = room();
    let late = op("k1", "n1", &["k2"]);
    assert_eq!(code(put(&mut r, "a", late.clone())), "dependency_rejected");
    // The refusal did not apply or reserve anything: not even the dependent's own key.
    assert_eq!(r.store().find("file_1", "a", "k1").unwrap(), None);
    put(&mut r, "a", op("k2", "n2", &[])).unwrap();
    assert_eq!(put(&mut r, "a", late).unwrap(), (2, true));
}

#[test]
fn an_op_that_depends_on_itself_is_refused_at_the_gate() {
    let err = Gate::new()
        .submit(&Principal::new("ws_1", "a"), vec![op("k1", "n1", &["k1"])])
        .unwrap_err();
    assert_eq!(err.code(), "invalid");
}

#[test]
fn a_cycle_can_never_be_applied() {
    // k1 needs k2 and k2 needs k1: whichever comes first names a key that is not applied yet, in
    // either order and in one batch or two, so neither is ever written.
    let mut r = room();
    let first = batch(
        &mut r,
        "a",
        vec![op("k1", "n1", &["k2"]), op("k2", "n2", &["k1"])],
    );
    assert_eq!(
        first.map_codes(),
        ["dependency_rejected", "dependency_rejected"]
    );
    assert_eq!(
        code(put(&mut r, "a", op("k2", "n2", &["k1"]))),
        "dependency_rejected"
    );
    assert_eq!(
        code(put(&mut r, "a", op("k1", "n1", &["k2"]))),
        "dependency_rejected"
    );
    assert_eq!(r.seq(), 0);
}

trait Codes {
    fn map_codes(&self) -> Vec<&'static str>;
}

impl Codes for Vec<Result<(u64, bool), Refusal>> {
    fn map_codes(&self) -> Vec<&'static str> {
        self.iter()
            .map(|x| x.as_ref().err().map_or("ok", Refusal::code))
            .collect()
    }
}

// A refused dependency stays unapplied; the refusal cascades.

#[test]
fn a_dependency_that_was_refused_stays_unapplied_and_so_does_what_depends_on_it() {
    let mut r = room();
    assert_eq!(
        code(put(&mut r, "a", Op::new("k1", bad("n1")))),
        "E_PARENT_NOT_FOUND"
    );
    assert_eq!(
        code(put(&mut r, "a", op("k2", "n2", &["k1"]))),
        "dependency_rejected"
    );
    // Cascade: k3 depends on k2, which was refused, so k3 is refused too.
    assert_eq!(
        put(&mut r, "a", op("k3", "n3", &["k2"])).unwrap_err(),
        Refusal::DependencyRejected { key: "k2".into() }
    );
    assert_eq!((r.seq(), r.nodes().len()), (0, 0));
}

#[test]
fn after_a_refusal_the_corrected_chain_is_accepted_in_order() {
    let mut r = room();
    put(&mut r, "a", Op::new("k1", bad("n1"))).unwrap_err();
    put(&mut r, "a", op("k2", "n2", &["k1"])).unwrap_err();
    // The key of a refused op is free: send k1 again, corrected, then its dependent.
    assert_eq!(put(&mut r, "a", op("k1", "n1", &[])).unwrap(), (1, true));
    assert_eq!(
        put(&mut r, "a", op("k2", "n2", &["k1"])).unwrap(),
        (2, true)
    );
}

// Order within a batch.

#[test]
fn in_a_batch_an_op_may_depend_on_an_earlier_op_of_the_same_batch() {
    let mut r = room();
    let res = batch(
        &mut r,
        "a",
        vec![
            op("k1", "n1", &[]),
            op("k2", "n2", &["k1"]),
            op("k3", "n3", &["k1", "k2"]),
        ],
    );
    assert_eq!(res.map_codes(), ["ok", "ok", "ok"]);
    assert_eq!(ids(&r), ["n1", "n2", "n3"]);
}

#[test]
fn in_a_batch_a_dependency_on_a_later_op_is_refused() {
    let mut r = room();
    let res = batch(
        &mut r,
        "a",
        vec![op("k1", "n1", &["k2"]), op("k2", "n2", &[])],
    );
    assert_eq!(res.map_codes(), ["dependency_rejected", "ok"]);
    assert_eq!(ids(&r), ["n2"]);
}

#[test]
fn in_a_batch_a_refused_op_refuses_the_ops_that_depend_on_it() {
    let mut r = room();
    let res = batch(
        &mut r,
        "a",
        vec![
            Op::new("k1", bad("n1")),
            op("k2", "n2", &["k1"]),
            op("k3", "n3", &["k2"]),
            op("k4", "n4", &[]),
        ],
    );
    assert_eq!(
        res.map_codes(),
        [
            "E_PARENT_NOT_FOUND",
            "dependency_rejected",
            "dependency_rejected",
            "ok"
        ]
    );
    assert_eq!(ids(&r), ["n4"]);
}

// The bound.

#[test]
fn exactly_the_most_dependencies_pass_the_gate_and_the_room_and_one_more_is_invalid() {
    let mut r = room();
    let keys: Vec<String> = (0..=MAX_OP_DEPS).map(|i| format!("d{i}")).collect();
    for k in &keys[..MAX_OP_DEPS] {
        put(&mut r, "a", op(k, k, &[])).unwrap();
    }
    let with = |n: usize| Op::new("last", add("last")).with_deps(keys[..n].iter().cloned());
    // 513 names (the 513th was never sent either): refused at the gate, before the room.
    let err = Gate::new()
        .submit(&Principal::new("ws_1", "a"), vec![with(MAX_OP_DEPS + 1)])
        .unwrap_err();
    assert_eq!(err.code(), "invalid");
    assert!(err.to_string().contains("512"), "{err}");
    assert_eq!(r.seq(), MAX_OP_DEPS as u64);
    assert_eq!(
        put(&mut r, "a", with(MAX_OP_DEPS)).unwrap().0,
        MAX_OP_DEPS as u64 + 1
    );
}

// How long a key is remembered.

#[test]
fn an_applied_key_is_remembered_across_a_restart_and_a_snapshot_and_a_long_log() {
    // Snapshot every 100 ops and more ops than one replay page: k0 is far behind the snapshot.
    let mut r = room().with_snapshot_every(100);
    let n = meldframe_host::REPLAY_PAGE * 2 + 50;
    for i in 0..n {
        put(&mut r, "a", op(&format!("k{i}"), &format!("n{i}"), &[])).unwrap();
    }
    assert!(r.store().latest_snapshot("file_1").unwrap().unwrap().seq > 400);
    let mut r = restart(r);
    assert_eq!(
        put(
            &mut r,
            "a",
            op("late", "late", &["k0", &format!("k{}", n - 1)])
        )
        .unwrap()
        .0,
        n as u64 + 1
    );
    // Still scoped to the actor after the restart.
    assert_eq!(
        code(put(&mut r, "b", op("x", "x", &["k0"]))),
        "dependency_rejected"
    );
}

#[test]
fn a_key_is_remembered_however_long_ago_it_was_applied() {
    // No time bound: other actors write for a long time in between and a resend still names k1.
    let mut r = room();
    put(&mut r, "a", op("k1", "n1", &[])).unwrap();
    for i in 0..300 {
        put(&mut r, "b", op(&format!("b{i}"), &format!("m{i}"), &[])).unwrap();
    }
    assert_eq!(put(&mut r, "a", op("k2", "n2", &["k1"])).unwrap().0, 302);
}

// Retries.

#[test]
fn a_retried_dependent_op_returns_its_number_and_writes_nothing() {
    let mut r = room();
    put(&mut r, "a", op("k1", "n1", &[])).unwrap();
    let dep = op("k2", "n2", &["k1"]);
    assert_eq!(put(&mut r, "a", dep.clone()).unwrap(), (2, true));
    assert_eq!(put(&mut r, "a", dep.clone()).unwrap(), (2, false));
    assert_eq!(r.seq(), 2);
    // Also after a restart, and a retry is a retry whatever deps it names: the key is the identity,
    // the stored op does not keep its dependencies.
    let mut r = restart(r);
    assert_eq!(put(&mut r, "a", dep).unwrap(), (2, false));
    assert_eq!(
        put(&mut r, "a", op("k2", "n2", &["nope"])).unwrap(),
        (2, false)
    );
    assert_eq!(ids(&r), ["n1", "n2"]);
}

#[test]
fn a_retried_dependent_op_with_another_command_is_a_reused_key() {
    let mut r = room();
    put(&mut r, "a", op("k1", "n1", &[])).unwrap();
    put(&mut r, "a", op("k2", "n2", &["k1"])).unwrap();
    assert_eq!(
        code(put(&mut r, "a", op("k2", "other", &["k1"]))),
        "invalid"
    );
}
