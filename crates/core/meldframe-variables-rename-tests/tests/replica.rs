//! A rename in replicas and a room (D8): every replica ends equal to the room whichever order the ops reach it, a
//! rename that the order makes invalid is refused or dropped, and an undo is built on the rename it reverses.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-variables-tests/tests/common/mod.rs"]
mod common;

use common::{appearance, bg, cmd};
use meldframe_host::{Gate, MemoryStore, Op, Principal, Refusal, Room, RoomError};
use meldframe_protocol::Command;
use meldframe_sync::{EventOutcome, Replica};
use serde_json::json;
use std::collections::VecDeque;

fn rename_collection(id: &str, name: &str) -> Command {
    cmd(json!({"type": "renameCollection", "id": id, "name": name}))
}

fn rename_mode(collection: &str, mode: &str, name: &str) -> Command {
    cmd(json!({"type": "renameMode", "collection": collection, "mode": mode, "name": name}))
}

fn rename_variable(id: &str, name: &str) -> Command {
    cmd(json!({"type": "renameVariable", "id": id, "name": name}))
}

#[allow(clippy::large_enum_variant)]
enum Msg {
    Event(u64, String, String, Command),
    Reject(String),
}

/// A room and two replicas, `a` and `b`, joined by queues: what the room accepts or refuses reaches each replica
/// only when the test delivers it, so two actors can edit against the same state.
struct Sim {
    room: Room<MemoryStore>,
    gate: Gate,
    reps: Vec<Replica>,
    inbox: Vec<VecDeque<Msg>>,
}

impl Sim {
    fn new() -> Sim {
        Sim {
            room: Room::open("ws_1", "file_1", MemoryStore::new()).unwrap(),
            gate: Gate::new(),
            reps: ["user:a", "user:b"]
                .iter()
                .map(|a| Replica::new(a, &a[5..], vec![], 0).unwrap())
                .collect(),
            inbox: vec![VecDeque::new(), VecDeque::new()],
        }
    }

    /// Collection `t` (light, dark) with `bg`, `fg` and a second collection `u`, made by a third actor and delivered:
    /// the histories of `a` and `b` start empty.
    fn seeded() -> Sim {
        let mut s = Sim::new();
        for (i, c) in [
            appearance(),
            bg(),
            cmd(json!({"type": "addVariable", "variable": {"id": "fg", "collection": "t", "name": "color/fg",
                "type": "color", "values": {"light": "#000000", "dark": "#ffffff"}}})),
            cmd(json!({"type": "addCollection", "collection": {"id": "u", "name": "other", "modes": [{"id": "m", "name": "m"}]}})),
        ]
        .into_iter()
        .enumerate()
        {
            s.submit(None, "user:seed", &format!("s{i}"), &c, &[]).unwrap();
        }
        s.deliver_all();
        s
    }

    /// Put one op through the gate and the room. An accepted op goes to every inbox; a refusal goes to the inbox of
    /// `who`, the replica that sent it.
    fn submit(
        &mut self,
        who: Option<usize>,
        actor: &str,
        key: &str,
        c: &Command,
        deps: &[String],
    ) -> Result<u64, Refusal> {
        let op = Op::new(key, c.clone()).with_deps(deps.to_vec());
        let admission = self
            .gate
            .submit(&Principal::new("ws_1", actor), vec![op])
            .unwrap()
            .admitted
            .remove(0);
        match self.room.accept(admission) {
            Ok(a) => {
                for q in &mut self.inbox {
                    q.push_back(Msg::Event(a.seq, actor.into(), key.into(), c.clone()));
                }
                Ok(a.seq)
            }
            Err(RoomError::Refused(r)) => {
                if let Some(w) = who {
                    self.inbox[w].push_back(Msg::Reject(key.into()));
                }
                Err(r)
            }
            Err(e) => panic!("{e}"),
        }
    }

    /// Send the outbox of replica `who` to the room, in order.
    fn send(&mut self, who: usize) -> Vec<Result<u64, Refusal>> {
        let actor = self.reps[who].actor().to_string();
        self.reps[who]
            .take_outbox()
            .into_iter()
            .map(|o| self.submit(Some(who), &actor, &o.key, &o.command, &o.deps))
            .collect()
    }

    /// Deliver what `who` has waiting; the outcome of each event.
    fn deliver(&mut self, who: usize) -> Vec<EventOutcome> {
        let mut out = Vec::new();
        while let Some(m) = self.inbox[who].pop_front() {
            match m {
                Msg::Event(seq, actor, key, c) => out.push(
                    self.reps[who]
                        .on_event_keyed(seq, &actor, Some(&key), &c)
                        .unwrap(),
                ),
                Msg::Reject(key) => {
                    self.reps[who].on_reject(&key);
                }
            }
        }
        out
    }

    fn deliver_all(&mut self) {
        self.deliver(0);
        self.deliver(1);
    }

    /// Every replica is consistent, and once nothing is pending it equals the room.
    fn converged(&self) {
        for r in &self.reps {
            r.check_consistency().unwrap();
            if r.pending_len() == 0 {
                assert_eq!(
                    r.vars(),
                    self.room.vars(),
                    "{} differs from the room",
                    r.actor()
                );
            }
        }
    }
}

fn name(r: &Replica, id: &str) -> String {
    r.vars().variable(id).unwrap().name.clone()
}

#[test]
fn a_rename_reaches_the_room_and_both_replicas() {
    let mut s = Sim::seeded();
    s.reps[0]
        .apply(&rename_variable("bg", "color/surface"))
        .unwrap();
    s.reps[0].apply(&rename_collection("t", "theme")).unwrap();
    s.reps[0].apply(&rename_mode("t", "dark", "Night")).unwrap();
    assert!(s.send(0).iter().all(Result::is_ok));
    s.deliver_all();
    s.converged();
    for r in &s.reps {
        let c = r.vars().collection("t").unwrap();
        assert_eq!(
            (name(r, "bg"), c.name.as_str(), c.modes[1].name.as_str()),
            ("color/surface".into(), "theme", "Night")
        );
    }
}

#[test]
fn two_renames_of_one_entity_end_with_the_one_the_room_ordered_last() {
    let mut s = Sim::seeded();
    s.reps[0].apply(&rename_variable("bg", "color/a")).unwrap();
    s.reps[1].apply(&rename_variable("bg", "color/b")).unwrap();
    s.send(0);
    s.send(1);
    s.deliver_all();
    s.converged();
    assert_eq!(name(&s.reps[0], "bg"), "color/b");
    assert_eq!(name(&s.reps[1], "bg"), "color/b");
}

#[test]
fn a_rename_to_a_name_the_other_actor_took_first_is_refused_and_both_replicas_converge() {
    let mut s = Sim::seeded();
    s.reps[0].apply(&rename_variable("bg", "color/x")).unwrap();
    s.reps[1].apply(&rename_variable("fg", "color/x")).unwrap();
    assert!(s.send(0)[0].is_ok());
    // `b` sends before it has seen the event of `a`: the room is the last writer and checks the name.
    match &s.send(1)[0] {
        Err(Refusal::Apply(e)) => assert_eq!(e.code(), "E_DUPLICATE_NAME"),
        other => panic!("{other:?}"),
    }
    let seen = s.deliver(1);
    assert_eq!(seen, vec![EventOutcome::Rebased { dropped: 1 }]);
    s.deliver(0);
    s.converged();
    assert_eq!(
        (name(&s.reps[1], "bg"), name(&s.reps[1], "fg")),
        ("color/x".into(), "color/fg".into())
    );
}

#[test]
fn a_pending_rename_that_a_remote_rename_makes_a_duplicate_is_dropped_by_the_replica() {
    let mut s = Sim::seeded();
    s.reps[0].apply(&rename_variable("bg", "color/x")).unwrap();
    s.reps[1].apply(&rename_variable("fg", "color/x")).unwrap();
    s.send(0);
    // The event of `a` reaches `b` before `b` sends: the pending rename no longer applies and is not sent.
    assert_eq!(s.deliver(1), vec![EventOutcome::Rebased { dropped: 1 }]);
    assert!(s.send(1).is_empty());
    s.deliver_all();
    s.converged();
}

#[test]
fn a_rename_of_what_the_other_actor_removed_is_refused_or_dropped() {
    let mut s = Sim::seeded();
    s.reps[0]
        .apply(&rename_variable("fg", "color/text"))
        .unwrap();
    s.reps[0].apply(&rename_mode("t", "dark", "Night")).unwrap();
    s.reps[0].apply(&rename_collection("u", "scratch")).unwrap();
    for c in [
        cmd(json!({"type": "removeVariable", "id": "fg"})),
        cmd(json!({"type": "removeMode", "collection": "t", "mode": "dark"})),
        cmd(json!({"type": "removeCollection", "id": "u"})),
    ] {
        s.reps[1].apply(&c).unwrap();
    }
    s.send(1);
    let refused = s.send(0);
    let codes: Vec<_> = refused
        .iter()
        .map(|r| match r {
            Err(Refusal::Apply(e)) => e.code(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        codes,
        [
            "E_VARIABLE_NOT_FOUND",
            "E_MODE_NOT_FOUND",
            "E_COLLECTION_NOT_FOUND"
        ]
    );
    s.deliver_all();
    s.converged();
    assert!(s.reps[0].vars().variable("fg").is_none());
}

#[test]
fn a_rename_and_a_change_of_values_or_modes_do_not_disturb_each_other() {
    let mut s = Sim::seeded();
    s.reps[0]
        .apply(&rename_variable("bg", "color/surface"))
        .unwrap();
    s.reps[0].apply(&rename_mode("t", "dark", "Night")).unwrap();
    s.reps[1]
        .apply(&cmd(
            json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}),
        ))
        .unwrap();
    s.reps[1]
        .apply(&cmd(
            json!({"type": "addMode", "collection": "t", "mode": {"id": "hc", "name": "hc"}}),
        ))
        .unwrap();
    s.reps[1].apply(&rename_collection("t", "theme")).unwrap();
    s.send(0);
    s.send(1);
    s.deliver_all();
    s.converged();
    let v = s.room.vars();
    assert_eq!(v.variable("bg").unwrap().name, "color/surface");
    assert_eq!(v.collection("t").unwrap().name, "theme");
    assert_eq!(v.collection("t").unwrap().modes.len(), 3);
}

#[test]
fn a_pending_rename_is_a_dependency_of_its_undo() {
    let mut s = Sim::seeded();
    s.reps[0]
        .apply(&rename_variable("bg", "color/surface"))
        .unwrap();
    s.reps[0]
        .apply(&rename_variable("bg", "color/panel"))
        .unwrap();
    assert!(s.reps[0].undo().unwrap());
    let out = s.reps[0].take_outbox();
    let keys: Vec<_> = out.iter().map(|o| o.key.clone()).collect();
    assert_eq!(out.len(), 3);
    assert!(
        out[2].deps.contains(&keys[1]),
        "the undo reads the name the second rename wrote: {:?}",
        out[2].deps
    );
    for (i, o) in out.iter().enumerate() {
        s.submit(Some(0), "user:a", &o.key, &o.command, &o.deps)
            .unwrap_or_else(|e| panic!("op {i}: {e:?}"));
    }
    assert_eq!(s.room.vars().variable("bg").unwrap().name, "color/surface");
}

#[test]
fn a_pending_collection_or_mode_rename_is_a_dependency_of_its_undo() {
    let mut s = Sim::seeded();
    s.reps[0].apply(&rename_collection("t", "theme")).unwrap();
    assert!(s.reps[0].undo().unwrap());
    s.reps[0].apply(&rename_mode("t", "dark", "Night")).unwrap();
    assert!(s.reps[0].undo().unwrap());
    let out = s.reps[0].take_outbox();
    assert_eq!(out.len(), 4);
    for (rename, undo) in [(0, 1), (2, 3)] {
        assert!(
            out[undo].deps.contains(&out[rename].key),
            "the undo of {:?} is built on it: {:?}",
            out[rename].command,
            out[undo].deps
        );
    }
    for o in &out {
        s.submit(Some(0), "user:a", &o.key, &o.command, &o.deps)
            .unwrap();
    }
    s.deliver_all();
    s.converged();
    let c = s.room.vars().collection("t").unwrap();
    assert_eq!(
        (c.name.as_str(), c.modes[1].name.as_str()),
        ("appearance", "dark")
    );
}

#[test]
fn my_undo_of_a_rename_is_gone_once_the_other_actor_renamed_the_same_variable() {
    let mut s = Sim::seeded();
    s.reps[0]
        .apply(&rename_variable("bg", "color/mine"))
        .unwrap();
    s.send(0);
    s.deliver_all();
    s.reps[1]
        .apply(&rename_variable("bg", "color/theirs"))
        .unwrap();
    s.send(1);
    s.deliver_all();
    assert!(
        !s.reps[0].can_undo(),
        "the step would overwrite the other actor"
    );
    assert!(!s.reps[0].undo().unwrap());
    assert!(s.send(0).is_empty());
    s.deliver_all();
    s.converged();
    assert_eq!(s.room.vars().variable("bg").unwrap().name, "color/theirs");
}

#[test]
fn the_undo_of_a_removal_brings_back_the_name_the_other_actor_gave_it_first() {
    let mut s = Sim::seeded();
    // `a` removes what `b` renames; `b`'s rename is ordered first, so the room accepts both.
    for c in [
        cmd(json!({"type": "removeVariable", "id": "fg"})),
        cmd(json!({"type": "removeMode", "collection": "t", "mode": "dark"})),
        cmd(json!({"type": "removeCollection", "id": "u"})),
    ] {
        s.reps[0].apply(&c).unwrap();
    }
    s.reps[1]
        .apply(&rename_variable("fg", "color/text"))
        .unwrap();
    s.reps[1].apply(&rename_mode("t", "dark", "Night")).unwrap();
    s.reps[1].apply(&rename_collection("u", "scratch")).unwrap();
    s.send(1);
    s.deliver(0);
    // The removal of the mode drops the values of `fg` too: put them back by the undo of the whole.
    assert!(s.send(0).iter().all(Result::is_ok));
    s.deliver_all();
    s.converged();
    for _ in 0..3 {
        assert!(s.reps[0].undo().unwrap());
    }
    s.send(0);
    s.deliver_all();
    s.converged();
    let v = s.room.vars();
    assert_eq!(v.variable("fg").unwrap().name, "color/text");
    assert_eq!(v.collection("u").unwrap().name, "scratch");
    assert_eq!(v.collection("t").unwrap().modes[1].name, "Night");
}
