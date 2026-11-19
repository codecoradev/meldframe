//! E5.2 and E5.3: revert an agent's batch and restore a checkpoint, as changes of the person that one undo takes back.

mod common;
use common::*;
use meldframe_app::{Action, CheckpointKind, Event, Property};
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
    assert_eq!((r1.x, r1.fill.as_str()), (50.0, "#00ff00"));
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
fn a_checkpoint_is_taken_before_a_revert() {
    let mut l = agent_then_person();
    let at = l.app.state().confirmed_seq;
    l.send(revert("agent:x", "b1"));
    let cps = &l.app.state().checkpoints;
    let c = cps
        .iter()
        .find(|c| c.kind == CheckpointKind::BeforeRevert)
        .expect("a checkpoint before the revert");
    assert_eq!(c.seq, at);
}

#[test]
fn a_checkpoint_is_taken_when_an_agent_run_starts_and_not_again_within_it() {
    let mut l = Loopback::live();
    let runs = |l: &Loopback| {
        l.app
            .state()
            .checkpoints
            .iter()
            .filter(|c| c.kind == CheckpointKind::AgentRun)
            .map(|c| c.seq)
            .collect::<Vec<_>>()
    };
    l.remote("agent:x", "b1:0", 1_000, set("r1", |p| p.x = Some(1.0)));
    assert_eq!(
        runs(&l),
        vec![0],
        "the point before the first op of the run"
    );
    l.remote("agent:x", "b1:1", 2_000, set("r1", |p| p.x = Some(2.0)));
    assert_eq!(runs(&l), vec![0]);
    l.remote("agent:x", "b2:0", 2_000_000, set("r1", |p| p.x = Some(3.0)));
    assert_eq!(runs(&l), vec![0, 2], "a new run after a long pause");
    // The person's own ops do not start a run.
    l.send(Action::Move {
        id: "t1".into(),
        x: 1.0,
        y: 1.0,
    });
    assert_eq!(runs(&l), vec![0, 2]);
}

#[test]
fn restoring_a_checkpoint_appends_a_change_of_the_person_that_undo_takes_back() {
    let mut l = Loopback::live();
    l.send(Action::CreateCheckpoint {
        label: "before the agent".into(),
    });
    let saved = l.app.state().checkpoints.last().unwrap().clone();
    assert_eq!(saved.kind, CheckpointKind::Manual);
    l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(50.0)));
    l.send(Action::SetProperty {
        id: "t1".into(),
        property: Property::Name("Headline".into()),
    });
    let head = l.seq;
    let effects = l.send(Action::RestoreCheckpoint { id: saved.id });
    assert_eq!(get(&l.app, "r1").x, 0.0);
    assert_eq!(get(&l.app, "t1").name, "");
    assert_eq!(sent(&effects).len(), 2, "two commands, appended to the log");
    assert!(l.seq > head, "the log only grew");
    assert!(
        l.app
            .state()
            .checkpoints
            .iter()
            .any(|c| c.kind == CheckpointKind::BeforeRestore),
        "a checkpoint is taken before a restore"
    );
    l.send(Action::Undo {});
    assert_eq!(get(&l.app, "t1").name, "Headline");
    assert_eq!(get(&l.app, "r1").x, 50.0);
}

#[test]
fn an_unknown_checkpoint_is_refused_and_a_label_is_limited() {
    let mut l = Loopback::live();
    l.send(Action::RestoreCheckpoint { id: "nope".into() });
    l.send(Action::CreateCheckpoint {
        label: "x".repeat(200),
    });
    assert_eq!(refused(&l), vec!["unknown_checkpoint", "limit"]);
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
    });
    l.events.extend(l.app.take_events());
    l.send(revert("agent:x", "b1"));
    assert_eq!(refused(&l), vec!["unknown_batch"]);
}

#[test]
fn the_new_actions_and_the_checkpoint_list_have_a_wire_shape() {
    let a: Action = serde_json::from_value(
        serde_json::json!({"type":"revertBatch","actor":"agent:x","keyPrefix":"b1"}),
    )
    .unwrap();
    assert!(matches!(a, Action::RevertBatch { .. }));
    assert!(
        serde_json::from_value::<Action>(
            serde_json::json!({"type":"createCheckpoint","label":"v1","x":1})
        )
        .is_err()
    );
    let mut l = Loopback::live();
    l.send(Action::CreateCheckpoint { label: "v1".into() });
    let v = serde_json::to_value(l.app.state()).unwrap();
    assert_eq!(v["checkpoints"][0]["kind"], "manual");
    assert_eq!(v["checkpoints"][0]["label"], "v1");
    assert!(v["checkpoints"][0].get("actor").is_none());
}
