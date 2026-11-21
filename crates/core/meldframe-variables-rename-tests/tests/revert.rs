//! A revert does not take a rename back (D8, ADR-0017): like every variable command it is listed as skipped, so that
//! `Event::Reverted` does not report a full revert that is not one, and the node part of the batch still reverts.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-variables-tests/tests/common/mod.rs"]
mod common;

use common::{add, appearance, bg, cmd};
use meldframe_history::FileState;
use meldframe_protocol::Command;
use meldframe_revert::{Batch, LoggedOp, SkipReason, Skipped, revert_batch};
use serde_json::json;

#[test]
fn the_renames_of_a_batch_are_listed_as_not_reverted_and_the_node_part_still_reverts() {
    let start = {
        let mut s = FileState::new();
        s.apply(&add("a", "rect", None)).unwrap();
        s.nodes().to_vec()
    };
    let ops: Vec<(&str, &str, Command)> = vec![
        ("human", "ui-1", appearance()),
        ("human", "ui-2", bg()),
        (
            "human",
            "ui-3",
            cmd(json!({"type": "updateNode", "id": "a", "patch": {"x": 1.0}})),
        ),
        (
            "agent:x",
            "b1:0",
            cmd(json!({"type": "updateNode", "id": "a", "patch": {"x": 9.0}})),
        ),
        (
            "agent:x",
            "b1:1",
            cmd(json!({"type": "renameVariable", "id": "bg", "name": "color/surface"})),
        ),
        (
            "agent:x",
            "b1:2",
            cmd(json!({"type": "renameMode", "collection": "t", "mode": "dark", "name": "Night"})),
        ),
        (
            "agent:x",
            "b1:3",
            cmd(json!({"type": "renameCollection", "id": "t", "name": "theme"})),
        ),
    ];
    let mut s = FileState::from_nodes_unchecked(start.clone());
    let log: Vec<LoggedOp> = ops
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
    let r = revert_batch(
        &start,
        &log,
        Batch {
            actor: "agent:x",
            key_prefix: "b1",
        },
    )
    .unwrap();
    let mut after = FileState::from_nodes_unchecked(s.nodes().to_vec());
    for c in &r.commands {
        after.replay_pending(c).expect("a revert applies whole");
    }
    assert_eq!(after.node("a").unwrap().x, 1.0);
    let skip = |node: &str| Skipped {
        node: node.into(),
        what: "variable".into(),
        reason: SkipReason::NotApplicable,
    };
    // The newest op first: a collection (a mode counts for its collection) and a variable, each listed once.
    assert_eq!(r.skipped, vec![skip("t"), skip("bg")]);
    // The names stay as the batch left them.
    assert_eq!(s.vars().variable("bg").unwrap().name, "color/surface");
}
