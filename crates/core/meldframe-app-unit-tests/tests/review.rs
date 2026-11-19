//! E5.2 and E5.3: revert an agent's batch and restore a checkpoint, as changes of the person that one undo takes back.

mod common;
use common::*;
use meldframe_app::{Action, CheckpointKind, Event, Frame, Property};
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
            .filter(|c| c.kind == CheckpointKind::OtherActorRun)
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

fn add(id: &str, kind: &str, parent: &str) -> Command {
    Command::AddNode {
        node: node(id, kind, Some(parent)),
        index: None,
    }
}

fn kinds(l: &Loopback, kind: CheckpointKind) -> usize {
    l.app
        .state()
        .checkpoints
        .iter()
        .filter(|c| c.kind == kind)
        .count()
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
fn a_restore_with_nothing_to_change_is_refused() {
    let mut l = Loopback::live();
    l.send(Action::CreateCheckpoint {
        label: "now".into(),
    });
    let id = l.app.state().checkpoints[0].id.clone();
    l.send(Action::RestoreCheckpoint { id });
    assert_eq!(refused(&l), vec!["nothing_to_restore"]);
}

#[test]
fn a_revert_or_restore_that_does_not_fit_the_queue_is_refused_whole() {
    let mut l = Loopback::live();
    l.send(Action::CreateCheckpoint { label: "v1".into() });
    let id = l.app.state().checkpoints[0].id.clone();
    l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(10.0)));
    l.remote("agent:x", "b1:1", 0, set("t1", |p| p.x = Some(10.0)));
    l.app.set_max_pending(1);
    let sent_before = l.app.state().pending.len();
    l.send(revert("agent:x", "b1"));
    l.send(Action::RestoreCheckpoint { id });
    assert_eq!(refused(&l), vec!["outbox_full", "outbox_full"]);
    assert_eq!(l.app.state().pending.len(), sent_before);
    assert_eq!((get(&l.app, "r1").x, get(&l.app, "t1").x), (10.0, 10.0));
    assert_eq!(
        kinds(&l, CheckpointKind::BeforeRevert),
        0,
        "no checkpoint for a refused revert"
    );
}

#[test]
fn a_refused_command_in_the_middle_of_a_restore_leaves_a_consistent_replica() {
    let mut l = Loopback::live();
    l.remote("agent:x", "b1:0", 0, add("hero", "frame", "root"));
    l.remote("agent:x", "b1:1", 0, add("pic", "rect", "hero"));
    l.send(Action::CreateCheckpoint {
        label: "with hero".into(),
    });
    let id = l.app.state().checkpoints.last().unwrap().id.clone();
    l.remote(
        "agent:x",
        "b2:0",
        0,
        Command::RemoveNode { id: "pic".into() },
    );
    l.remote(
        "agent:x",
        "b2:1",
        0,
        Command::RemoveNode { id: "hero".into() },
    );
    let effects = l.app.dispatch(Action::RestoreCheckpoint { id });
    let ops: Vec<_> = effects
        .iter()
        .flat_map(|e| match e {
            meldframe_app::Effect::SendOps { ops } => ops.clone(),
            _ => vec![],
        })
        .collect();
    assert_eq!(ops.len(), 2, "add hero, then add pic under it");
    assert!(l.app.nodes().iter().any(|n| n.id == "pic"));
    // The server refuses the frame; the add of its child depended on it and falls too.
    l.app.dispatch(Action::Frame {
        now_ms: 0,
        frame: Frame::Rejected {
            key: ops[0].key.clone(),
            code: "forbidden".into(),
            retryable: false,
        },
    });
    l.events.extend(l.app.take_events());
    assert!(
        !l.app
            .nodes()
            .iter()
            .any(|n| n.id == "hero" || n.id == "pic")
    );
    assert!(
        l.app.state().pending.is_empty(),
        "nothing waits on the refused op"
    );
    assert!(
        l.events
            .iter()
            .any(|e| matches!(e, Event::OpRejected { .. }))
    );
    assert!(
        l.events
            .iter()
            .any(|e| matches!(e, Event::Restored { requested: 2, .. }))
    );
}

#[test]
fn a_batch_cut_by_the_end_of_the_record_is_told_apart_from_an_unknown_one() {
    let mut l = Loopback::live();
    l.send(Action::CreateCheckpoint { label: "v0".into() });
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
    assert_eq!(
        l.app.state().checkpoints.len(),
        3,
        "the saved one and the two runs"
    );
    l.remote("agent:x", "new:0", 0, set("r1", |p| p.x = Some(2.0)));
    assert!(
        l.app.state().checkpoints.is_empty(),
        "the checkpoints before the base are dropped"
    );
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
    l.send(Action::RestoreCheckpoint {
        id: "c0-cp1".into(),
    });
    assert_eq!(refused(&l).last().unwrap(), "unknown_checkpoint");
}

#[test]
fn past_the_cap_the_checkpoints_the_app_took_go_before_the_saved_ones() {
    let mut l = Loopback::live();
    for i in 0..40 {
        l.send(Action::CreateCheckpoint {
            label: format!("saved {i}"),
        });
    }
    for run in 0..80u64 {
        l.remote(
            "agent:x",
            &format!("r{run}:0"),
            run * 1_000_000,
            set("r1", |p| p.x = Some(run as f64)),
        );
    }
    let cps = &l.app.state().checkpoints;
    assert_eq!(cps.len(), 100);
    assert_eq!(
        cps.iter()
            .filter(|c| c.kind == CheckpointKind::Manual)
            .count(),
        40
    );
    // Only the newest runs are left, and the newest checkpoint is the last one taken.
    assert_eq!(cps.last().unwrap().seq, 79, "the point before the 80th run");
}

#[test]
fn with_only_saved_checkpoints_the_oldest_goes() {
    let mut l = Loopback::live();
    for i in 0..101 {
        l.send(Action::CreateCheckpoint {
            label: format!("saved {i}"),
        });
    }
    let cps = &l.app.state().checkpoints;
    assert_eq!(cps.len(), 100);
    assert_eq!(cps[0].label, "saved 1");
}
