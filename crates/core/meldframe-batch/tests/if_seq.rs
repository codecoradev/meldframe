//! The dry run of a batch and `Room::accept` must agree on which ids an `ifSeq` guard covers, for every command.
//!
//! Each case runs one command and then probes, for every id of a small universe, whether an `ifSeq` taken before
//! that command conflicts on the id: once through the room (the command is accepted, then a probe op with an old
//! `ifSeq`) and once through the dry run (the command and the probe in one batch). Both answers must equal the ids
//! the case lists by hand, so a change to one of the two counts, or to both, fails here.

use meldframe_batch::{Stop, check};
use meldframe_host::{
    Admission, Gate, MemoryStore, NodeFacts, Op, Principal, Refusal, Room, RoomError,
};
use meldframe_protocol::Command;
use serde_json::{Value, json};

fn cmd(v: Value) -> Command {
    serde_json::from_value(v).unwrap()
}

/// Every delete is small, so that `removeNode` of a leaf passes the gate as a Change.
struct SmallDeletes;

impl NodeFacts for SmallDeletes {
    fn delete_is_small(&self, _: &str) -> bool {
        true
    }
}

fn admit(op: Op) -> Admission {
    Gate::new()
        .submit_with(&Principal::new("ws_1", "a"), vec![op], &SmallDeletes)
        .unwrap()
        .admitted
        .remove(0)
}

fn node(id: &str, kind: &str, parent: Option<&str>) -> Command {
    cmd(json!({"type": "addNode", "node": {"id": id, "type": kind, "parentId": parent}}))
}

/// Frames `f` and `g`, rects `a` and `b` in `f`, the collections `t` (modes light and dark) and `u` (one mode, no
/// variable), and the colour variable `bg` of `t`.
fn setup() -> Vec<Command> {
    vec![
        node("f", "frame", None),
        node("g", "frame", None),
        node("a", "rect", Some("f")),
        node("b", "rect", Some("f")),
        cmd(
            json!({"type": "addCollection", "collection": {"id": "t", "name": "appearance",
            "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}}),
        ),
        cmd(
            json!({"type": "addCollection", "collection": {"id": "u", "name": "spare",
            "modes": [{"id": "m", "name": "m"}]}}),
        ),
        cmd(
            json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t",
            "name": "color/bg", "type": "color", "values": {"light": "#ffffff", "dark": "#000000"}}}),
        ),
    ]
}

/// Every id a probe can name: nodes, then variables and collections as `ifSeq` names them.
const UNIVERSE: [&str; 10] = [
    "f", "g", "a", "b", "new", "$var:bg", "$var:fg", "$col:t", "$col:u", "$col:n",
];

/// A command that touches exactly `id`, whatever the file holds: the guard is checked before the command applies.
fn probe(id: &str) -> Command {
    if let Some(var) = id.strip_prefix("$var:") {
        cmd(json!({"type": "setVariableValue", "id": var, "mode": "light", "value": "#123456"}))
    } else if let Some(col) = id.strip_prefix("$col:") {
        cmd(json!({"type": "renameCollection", "id": col, "name": "probe"}))
    } else {
        cmd(json!({"type": "updateNode", "id": id, "patch": {"name": "probe"}}))
    }
}

fn room() -> Room<MemoryStore> {
    let mut r = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    for (i, c) in setup().into_iter().enumerate() {
        r.accept(admit(Op::new(format!("s{i}"), c))).unwrap();
    }
    r
}

/// True when the room refuses `probe(id)` with an `ifSeq` older than the accepted `command`, as a conflict on `id`.
fn room_conflicts(command: &Command, id: &str) -> bool {
    let mut r = room();
    let before = r.seq();
    r.accept(admit(Op::new("c", command.clone()))).unwrap();
    let op = Op::new("p", probe(id)).with_if_seq(before);
    match r.accept(admit(op)) {
        Err(RoomError::Refused(Refusal::Conflict { node_id })) => {
            assert_eq!(node_id, id);
            true
        }
        _ => false,
    }
}

/// The same question put to the dry run: `command` and the probe in one batch, the probe guarded by the seq
/// before the batch.
fn dry_run_conflicts(command: &Command, id: &str) -> bool {
    let r = room();
    let ops = [
        Op::new("c", command.clone()),
        Op::new("p", probe(id)).with_if_seq(r.seq()),
    ];
    match check(&r, "a", &ops, &[]) {
        Err(Stop::Refused {
            index: 1,
            refusal: Refusal::Conflict { node_id },
        }) => {
            assert_eq!(node_id, id);
            true
        }
        _ => false,
    }
}

/// The place of a variant in the table below; the match has no wildcard, so a new command fails to compile here
/// until it has a case.
fn variant(c: &Command) -> usize {
    match c {
        Command::UpdateNode { .. } => 0,
        Command::AddNode { .. } => 1,
        Command::RemoveNode { .. } => 2,
        Command::Reparent { .. } => 3,
        Command::Reorder { .. } => 4,
        Command::AddCollection { .. } => 5,
        Command::RemoveCollection { .. } => 6,
        Command::RenameCollection { .. } => 7,
        Command::AddMode { .. } => 8,
        Command::RemoveMode { .. } => 9,
        Command::RenameMode { .. } => 10,
        Command::AddVariable { .. } => 11,
        Command::RemoveVariable { .. } => 12,
        Command::RenameVariable { .. } => 13,
        Command::SetVariableValue { .. } => 14,
    }
}

const VARIANTS: usize = 15;

/// One valid command of each variant, with the ids it covers.
fn cases() -> Vec<(Command, Vec<&'static str>)> {
    vec![
        (
            cmd(json!({"type": "updateNode", "id": "a", "patch": {"name": "x"}})),
            vec!["a"],
        ),
        (node("new", "rect", Some("f")), vec!["new"]),
        (cmd(json!({"type": "removeNode", "id": "b"})), vec!["b"]),
        (
            cmd(json!({"type": "reparent", "id": "a", "parentId": "g"})),
            vec!["a", "g"],
        ),
        (
            cmd(json!({"type": "reorder", "id": "b", "before": "a"})),
            vec!["b"],
        ),
        (
            cmd(
                json!({"type": "addCollection", "collection": {"id": "n", "name": "fresh",
                "modes": [{"id": "m", "name": "m"}]}}),
            ),
            vec!["$col:n"],
        ),
        (
            cmd(json!({"type": "removeCollection", "id": "u"})),
            vec!["$col:u"],
        ),
        (
            cmd(json!({"type": "renameCollection", "id": "t", "name": "theme"})),
            vec!["$col:t"],
        ),
        (
            cmd(
                json!({"type": "addMode", "collection": "t", "mode": {"id": "mid", "name": "mid"}}),
            ),
            vec!["$col:t"],
        ),
        (
            cmd(json!({"type": "removeMode", "collection": "t", "mode": "dark"})),
            vec!["$col:t"],
        ),
        (
            cmd(json!({"type": "renameMode", "collection": "t", "mode": "dark", "name": "night"})),
            vec!["$col:t"],
        ),
        (
            cmd(
                json!({"type": "addVariable", "variable": {"id": "fg", "collection": "t",
                "name": "color/fg", "type": "color",
                "values": {"light": "#000000", "dark": "#ffffff"}}}),
            ),
            vec!["$var:fg"],
        ),
        (
            cmd(json!({"type": "removeVariable", "id": "bg"})),
            vec!["$var:bg"],
        ),
        (
            cmd(json!({"type": "renameVariable", "id": "bg", "name": "color/surface"})),
            vec!["$var:bg"],
        ),
        (
            cmd(
                json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}),
            ),
            vec!["$var:bg"],
        ),
    ]
}

#[test]
fn the_cases_cover_every_command() {
    let mut seen: Vec<usize> = cases().iter().map(|(c, _)| variant(c)).collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..VARIANTS).collect::<Vec<_>>());
}

#[test]
fn every_case_is_a_valid_batch() {
    let r = room();
    for (command, _) in cases() {
        let ops = [Op::new("c", command.clone())];
        assert_eq!(check(&r, "a", &ops, &[]), Ok(()), "{command:?}");
    }
}

#[test]
fn the_dry_run_and_the_room_agree_on_the_ids_an_if_seq_covers() {
    for (command, covers) in cases() {
        for id in UNIVERSE {
            let want = covers.contains(&id);
            assert_eq!(
                room_conflicts(&command, id),
                want,
                "Room::accept, {command:?}, id {id}"
            );
            assert_eq!(
                dry_run_conflicts(&command, id),
                want,
                "dry run, {command:?}, id {id}"
            );
        }
    }
}
