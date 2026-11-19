//! Revert of a batch: only what the actor still wrote last comes back, as one list of commands.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_history::FileState;
use meldframe_protocol::{Command, Node, NodePatch};
use meldframe_revert::{Batch, LoggedOp, RevertError, SkipReason, revert_batch};

fn add(id: &str, kind: &str, parent: Option<&str>) -> Command {
    let node: Node =
        serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent}))
            .unwrap();
    Command::AddNode { node, index: None }
}

fn upd(id: &str, f: impl FnOnce(&mut NodePatch)) -> Command {
    let mut patch = NodePatch::default();
    f(&mut patch);
    Command::UpdateNode {
        id: id.into(),
        patch,
    }
}

fn remove(id: &str) -> Command {
    Command::RemoveNode { id: id.into() }
}

/// `root > a, b` built through commands, so every node has a `pos`.
fn before() -> Vec<Node> {
    let mut s = FileState::new();
    for c in [
        add("root", "frame", None),
        add("a", "rect", Some("root")),
        add("b", "rect", Some("root")),
    ] {
        s.apply(&c).unwrap();
    }
    s.nodes().to_vec()
}

/// The log of `ops` (actor, key, command) from seq 1, and the tree after them.
fn run(start: &[Node], ops: Vec<(&str, &str, Command)>) -> (Vec<LoggedOp>, Vec<Node>) {
    let mut s = FileState::from_nodes_unchecked(start.to_vec());
    let log = ops
        .into_iter()
        .enumerate()
        .map(|(i, (actor, key, command))| {
            s.replay_pending(&command).unwrap();
            LoggedOp {
                seq: i as u64 + 1,
                actor: actor.into(),
                key: key.into(),
                command,
            }
        })
        .collect();
    (log, s.nodes().to_vec())
}

fn after_revert(current: &[Node], commands: &[Command]) -> FileState {
    let mut s = FileState::from_nodes_unchecked(current.to_vec());
    for c in commands {
        s.replay_pending(c).expect("a revert applies whole");
    }
    s
}

fn agent(prefix: &str) -> Batch<'_> {
    Batch {
        actor: "agent:x",
        key_prefix: prefix,
    }
}

#[test]
fn reverts_every_field_the_agent_wrote_last() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", upd("a", |p| p.x = Some(50.0))),
            (
                "agent:x",
                "b1:1",
                upd("a", |p| p.fill = Some("#ff0000".into())),
            ),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    let s = after_revert(&now, &r.commands);
    let a = s.node("a").unwrap();
    assert_eq!(a.x, 0.0);
    assert_ne!(a.fill, "#ff0000");
    assert!(r.skipped.is_empty());
}

#[test]
fn a_field_the_person_wrote_afterwards_stays() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            (
                "agent:x",
                "b1:0",
                upd("a", |p| {
                    p.x = Some(50.0);
                    p.fill = Some("#ff0000".into());
                }),
            ),
            (
                "human",
                "ui-1",
                upd("a", |p| p.fill = Some("#00ff00".into())),
            ),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    let a = after_revert(&now, &r.commands).node("a").unwrap().clone();
    assert_eq!(a.x, 0.0, "x is still the agent's: reverted");
    assert_eq!(a.fill, "#00ff00", "fill is the person's now: kept");
    assert_eq!(
        r.skipped
            .iter()
            .map(|s| (s.node.as_str(), s.what.as_str(), &s.reason))
            .collect::<Vec<_>>(),
        vec![("a", "fill", &SkipReason::ChangedSince)]
    );
}

#[test]
fn a_field_another_agent_wrote_afterwards_stays() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", upd("a", |p| p.x = Some(50.0))),
            ("agent:y", "c1:0", upd("a", |p| p.x = Some(70.0))),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    assert!(r.commands.is_empty());
    assert_eq!(after_revert(&now, &r.commands).node("a").unwrap().x, 70.0);
}

#[test]
fn two_writes_of_one_field_in_the_batch_go_back_to_the_value_before_the_batch() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", upd("a", |p| p.x = Some(10.0))),
            ("agent:x", "b1:1", upd("a", |p| p.x = Some(20.0))),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    assert_eq!(after_revert(&now, &r.commands).node("a").unwrap().x, 0.0);
}

#[test]
fn a_batch_is_the_key_prefix_and_not_a_longer_one() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", upd("a", |p| p.x = Some(10.0))),
            ("agent:x", "b10:0", upd("b", |p| p.x = Some(20.0))),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    let s = after_revert(&now, &r.commands);
    assert_eq!(s.node("a").unwrap().x, 0.0);
    assert_eq!(s.node("b").unwrap().x, 20.0);
}

#[test]
fn a_batch_nobody_wrote_is_unknown() {
    let start = before();
    let (log, _) = run(
        &start,
        vec![("human", "b1:0", upd("a", |p| p.x = Some(1.0)))],
    );
    assert_eq!(
        revert_batch(&start, &log, agent("b1")).unwrap_err(),
        RevertError::NoSuchBatch
    );
}

#[test]
fn nodes_the_batch_added_are_removed() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", add("hero", "frame", Some("root"))),
            ("agent:x", "b1:1", add("title", "text", Some("hero"))),
            (
                "agent:x",
                "b1:2",
                upd("title", |p| p.text = Some("Hi".into())),
            ),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    let s = after_revert(&now, &r.commands);
    assert!(s.node("hero").is_none() && s.node("title").is_none());
    assert_eq!(s.nodes().len(), start.len());
}

#[test]
fn a_node_the_person_edited_after_the_agent_added_it_stays() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", add("hero", "frame", Some("root"))),
            (
                "human",
                "ui-1",
                upd("hero", |p| p.fill = Some("#00ff00".into())),
            ),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    assert!(after_revert(&now, &r.commands).node("hero").is_some());
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(r.skipped[0].reason, SkipReason::ChangedSince);
}

#[test]
fn a_child_the_person_added_under_an_agent_node_keeps_the_node() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", add("hero", "frame", Some("root"))),
            ("human", "ui-1", add("mine", "rect", Some("hero"))),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    let s = after_revert(&now, &r.commands);
    assert!(s.node("hero").is_some() && s.node("mine").is_some());
}

#[test]
fn a_node_the_batch_removed_comes_back_with_its_subtree() {
    let start = {
        let mut s = FileState::from_nodes_unchecked(before());
        s.apply(&add("card", "frame", Some("root"))).unwrap();
        s.apply(&add("label", "text", Some("card"))).unwrap();
        s.nodes().to_vec()
    };
    let (log, now) = run(&start, vec![("agent:x", "b1:0", remove("card"))]);
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    let s = after_revert(&now, &r.commands);
    assert!(s.node("card").is_some() && s.node("label").is_some());
    assert_eq!(s.node("label").unwrap().parent_id.as_deref(), Some("card"));
}

#[test]
fn a_move_goes_back_unless_someone_moved_the_node_since() {
    let start = before();
    let mv = |id: &str| Command::Reorder {
        id: id.into(),
        before: None,
        after: None,
        pos: None,
    };
    // `a` goes to the front of its siblings.
    let (log, now) = run(&start, vec![("agent:x", "b1:0", mv("a"))]);
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    let s = after_revert(&now, &r.commands);
    assert_eq!(
        s.node("a").unwrap().pos,
        start.iter().find(|n| n.id == "a").unwrap().pos
    );
    // The person moved it again afterwards: it stays.
    let (log, now) = run(
        &start,
        vec![("agent:x", "b1:0", mv("a")), ("human", "ui-1", mv("a"))],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    assert!(r.commands.is_empty());
    assert_eq!(
        r.skipped,
        vec![meldframe_revert::Skipped {
            node: "a".into(),
            what: "place".into(),
            reason: SkipReason::ChangedSince
        }]
    );
    let moved = |nodes: &[Node]| nodes.iter().find(|n| n.id == "a").unwrap().pos.clone();
    assert_eq!(moved(after_revert(&now, &r.commands).nodes()), moved(&now));
}

#[test]
fn a_revert_is_one_list_that_applies_whole() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", add("hero", "frame", Some("root"))),
            ("agent:x", "b1:1", upd("a", |p| p.x = Some(9.0))),
            ("agent:x", "b1:2", remove("b")),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    // `after_revert` panics when a command of the list does not apply.
    let s = after_revert(&now, &r.commands);
    assert!(r.skipped.is_empty());
    let canon = |nodes: &[Node]| {
        let mut v: Vec<(String, Option<String>, String, f64)> = nodes
            .iter()
            .map(|n| (n.id.clone(), n.parent_id.clone(), n.pos.clone(), n.x))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    assert_eq!(
        canon(s.nodes()),
        canon(&start),
        "the tree is as before the batch"
    );
}

#[test]
fn a_later_batch_of_the_same_actor_keeps_the_field_it_wrote() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", upd("a", |p| p.x = Some(10.0))),
            ("agent:x", "b2:0", upd("a", |p| p.x = Some(20.0))),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    assert!(r.commands.is_empty());
    assert_eq!(after_revert(&now, &r.commands).node("a").unwrap().x, 20.0);
    assert_eq!(
        r.skipped,
        vec![meldframe_revert::Skipped {
            node: "a".into(),
            what: "x".into(),
            reason: SkipReason::ChangedSince
        }]
    );
}

#[test]
fn a_node_the_batch_added_and_someone_else_removed_is_listed_as_not_applicable() {
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            ("agent:x", "b1:0", add("hero", "rect", Some("root"))),
            ("human", "ui-1", remove("hero")),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    assert!(r.commands.is_empty());
    assert_eq!(after_revert(&now, &r.commands).nodes().len(), start.len());
    assert_eq!(
        r.skipped,
        vec![meldframe_revert::Skipped {
            node: "hero".into(),
            what: "node".into(),
            reason: SkipReason::NotApplicable
        }]
    );
}

#[test]
fn a_batch_with_only_some_of_its_ops_in_the_log_reverts_those_and_the_key_says_so() {
    // Op `b1:0` is not in the log (folded out, or refused): the rest reverts, and `batch_index` tells the caller.
    let start = before();
    let (log, now) = run(
        &start,
        vec![("agent:x", "b1:1", upd("a", |p| p.x = Some(5.0)))],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    assert_eq!(after_revert(&now, &r.commands).node("a").unwrap().x, 0.0);
    assert_eq!(meldframe_revert::batch_index("b1:1"), Some(("b1", 1)));
    assert_eq!(meldframe_revert::batch_index("ui-1"), None);
    assert_eq!(meldframe_revert::batch_index("b:x:7"), Some(("b:x", 7)));
}

fn cmd(v: serde_json::Value) -> Command {
    serde_json::from_value(v).unwrap()
}

#[test]
fn a_variable_command_of_the_batch_is_listed_as_not_reverted_and_the_node_part_still_reverts() {
    use serde_json::json;
    let start = before();
    let (log, now) = run(
        &start,
        vec![
            (
                "human",
                "ui-1",
                cmd(
                    json!({"type": "addCollection", "collection": {"id": "c", "name": "c", "modes": [{"id": "m", "name": "m"}]}}),
                ),
            ),
            (
                "human",
                "ui-2",
                cmd(
                    json!({"type": "addVariable", "variable": {"id": "v", "collection": "c", "name": "color/x", "type": "color", "values": {"m": "#102030"}}}),
                ),
            ),
            (
                "human",
                "ui-3",
                cmd(json!({"type": "updateNode", "id": "a", "patch": {"x": 1.0}})),
            ),
            // The batch: change a node field and add another variable.
            (
                "agent:x",
                "b1:0",
                cmd(json!({"type": "updateNode", "id": "a", "patch": {"x": 9.0}})),
            ),
            (
                "agent:x",
                "b1:1",
                cmd(
                    json!({"type": "addVariable", "variable": {"id": "w", "collection": "c", "name": "color/y", "type": "color", "values": {"m": "#203040"}}}),
                ),
            ),
        ],
    );
    let r = revert_batch(&start, &log, agent("b1")).unwrap();
    // The node part comes back; the variable stays where it is and the revert says so.
    let s = after_revert(&now, &r.commands);
    assert_eq!(s.node("a").unwrap().x, 1.0);
    assert_eq!(
        r.skipped,
        vec![meldframe_revert::Skipped {
            node: "w".into(),
            what: "variable".into(),
            reason: SkipReason::NotApplicable
        }]
    );
}
