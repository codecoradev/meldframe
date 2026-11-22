//! E5.2: revert an agent's batch, as a change of the person that one undo takes back. Where the server keeps
//! checkpoints the revert waits for its row: `revert_checkpoint.rs`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, Event, Property};
use meldframe_protocol::{Command, NodePatch};

fn set(id: &str, f: impl FnOnce(&mut NodePatch)) -> Command {
    let mut patch = NodePatch::default();
    f(&mut patch);
    Command::UpdateNode {
        id: id.into(),
        patch,
    }
}

fn revert(actor: &str, prefix: &str) -> Action {
    Action::RevertBatch {
        actor: actor.into(),
        key_prefix: prefix.into(),
    }
}

fn refused(l: &Loopback) -> Vec<String> {
    l.events
        .iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code.clone()),
            _ => None,
        })
        .collect()
}

/// The agent moves `r1` and paints it; the person then paints it another colour.
fn agent_then_person() -> Loopback {
    let mut l = Loopback::live();
    l.remote(
        "agent:x",
        "b1:0",
        0,
        set("r1", |p| {
            p.x = Some(50.0);
            p.fill = Some("#ff0000".into());
        }),
    );
    l.send(Action::SetProperty {
        id: "r1".into(),
        property: Property::Fill("#00ff00".into()),
    });
    l
}

#[test]
fn a_revert_restores_what_the_agent_still_wrote_last_as_one_step_of_the_person() {
    let mut l = agent_then_person();
    let effects = l.send(revert("agent:x", "b1"));
    let r1 = get(&l.app, "r1");
    assert_eq!(r1.x, 0.0, "the agent's move is taken back");
    assert_eq!(r1.fill, "#00ff00", "the person's colour stays");
    let ops = sent(&effects);
    assert_eq!(ops.len(), 1, "one command: only x was still the agent's");
    // One undo takes the whole revert back, and only the revert.
    l.send(Action::Undo {});
    let r1 = get(&l.app, "r1");
    assert_eq!(
        (r1.x, r1.fill.lit().map(String::as_str).unwrap_or("")),
        (50.0, "#00ff00")
    );
    assert!(l.events.iter().any(|e| matches!(e, Event::Reverted { .. })));
}

#[test]
fn a_revert_of_a_batch_nobody_wrote_is_refused() {
    let mut l = agent_then_person();
    l.send(revert("agent:x", "nope"));
    assert_eq!(refused(&l), vec!["unknown_batch"]);
    assert_eq!(get(&l.app, "r1").x, 50.0);
}

#[test]
fn a_revert_with_nothing_left_to_take_back_is_refused() {
    let mut l = Loopback::live();
    l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(50.0)));
    l.send(Action::Move {
        id: "r1".into(),
        x: 7.0,
        y: 0.0,
    });
    l.send(revert("agent:x", "b1"));
    assert_eq!(refused(&l), vec!["nothing_to_revert"]);
}

#[test]
fn a_reload_forgets_the_batches_before_it() {
    let mut l = agent_then_person();
    let nodes = l.app.nodes().to_vec();
    l.app.dispatch(Action::Reload {
        now_ms: 0,
        nodes,
        seq: l.seq,
        applied: vec![],
        variables: Default::default(),
    });
    l.events.extend(l.app.take_events());
    l.send(revert("agent:x", "b1"));
    assert_eq!(refused(&l), vec!["unknown_batch"]);
}

#[test]
fn a_later_batch_of_the_same_agent_keeps_what_it_wrote() {
    let mut l = Loopback::live();
    l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(10.0)));
    l.remote("agent:x", "b2:0", 1, set("r1", |p| p.x = Some(20.0)));
    l.send(revert("agent:x", "b1"));
    assert_eq!(refused(&l), vec!["nothing_to_revert"]);
    assert_eq!(get(&l.app, "r1").x, 20.0);
}

#[test]
fn a_partial_batch_reverts_its_fields_and_lists_the_rest() {
    let mut l = Loopback::live();
    l.remote(
        "agent:x",
        "b1:0",
        0,
        set("r1", |p| {
            p.x = Some(10.0);
            p.fill = Some("#111111".into());
        }),
    );
    l.send(Action::SetProperty {
        id: "r1".into(),
        property: Property::Fill("#222222".into()),
    });
    l.send(revert("agent:x", "b1"));
    let (requested, skipped) = l
        .events
        .iter()
        .find_map(|e| match e {
            Event::Reverted { requested, skipped } => Some((*requested, skipped.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(requested, 1);
    assert_eq!(skipped.len(), 1);
    assert_eq!(
        (skipped[0].node.as_str(), skipped[0].what.as_str()),
        ("r1", "fill")
    );
    assert_eq!(get(&l.app, "r1").x, 0.0);
    assert_eq!(get(&l.app, "r1").fill, "#222222");
}

#[test]
fn a_pending_op_of_the_person_counts_as_the_last_writer() {
    let mut l = Loopback::live();
    l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(10.0)));
    // The person moves it; the server has not confirmed that yet.
    l.app.dispatch(Action::Move {
        id: "r1".into(),
        x: 7.0,
        y: 0.0,
    });
    l.events.extend(l.app.take_events());
    assert_eq!(l.app.state().pending.len(), 1);
    l.send(revert("agent:x", "b1"));
    assert_eq!(refused(&l), vec!["nothing_to_revert"]);
    assert_eq!(
        get(&l.app, "r1").x,
        7.0,
        "the unconfirmed move is not overwritten"
    );
}

#[test]
fn a_revert_that_does_not_fit_the_queue_is_refused_whole() {
    let mut l = Loopback::live();
    l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(10.0)));
    l.remote("agent:x", "b1:1", 0, set("t1", |p| p.x = Some(10.0)));
    l.app.set_max_pending(1);
    let sent_before = l.app.state().pending.len();
    l.send(revert("agent:x", "b1"));
    assert_eq!(refused(&l), vec!["outbox_full"]);
    assert_eq!(l.app.state().pending.len(), sent_before);
    assert_eq!((get(&l.app, "r1").x, get(&l.app, "t1").x), (10.0, 10.0));
}

#[test]
fn a_batch_cut_by_the_end_of_the_record_is_told_apart_from_an_unknown_one() {
    let mut l = Loopback::live();
    // `old:0` is the first op (seq 1) and goes into the fold of the first 256 ops; `old:1` is seq 257 and stays.
    l.remote("agent:x", "old:0", 0, set("r1", |p| p.x = Some(1.0)));
    for i in 0..255 {
        l.remote(
            "agent:y",
            &format!("f{i}:0"),
            0,
            set("t1", |p| p.y = Some(i as f64)),
        );
    }
    l.remote("agent:x", "old:1", 0, set("t1", |p| p.x = Some(1.0)));
    for i in 0..1_998 {
        l.remote(
            "agent:y",
            &format!("g{i}:0"),
            0,
            set("t1", |p| p.y = Some(i as f64)),
        );
    }
    assert_eq!(l.seq, 2_255, "one op short of the fold");
    l.remote("agent:x", "new:0", 0, set("r1", |p| p.x = Some(2.0)));
    l.send(revert("agent:x", "old"));
    l.send(revert("agent:x", "new"));
    l.send(revert("agent:x", "nope"));
    assert_eq!(refused(&l), vec!["batch_incomplete", "unknown_batch"]);
    assert_eq!(
        get(&l.app, "r1").x,
        1.0,
        "the batch inside the record reverts"
    );
    assert_eq!(get(&l.app, "t1").x, 1.0, "the cut batch is left alone");
}
