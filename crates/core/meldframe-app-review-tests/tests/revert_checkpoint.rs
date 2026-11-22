//! E5.2, E5.3, ADR-0023: a revert and a restore wait for the server's row before they change the file, a restore works
//! from the file as the server had it at the checkpoint (so it still works after a reload), and one undo takes either
//! back. The server is scripted (`scripted/mod.rs`).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
mod scripted;
use common::*;
use meldframe_app::{
    Action, CheckpointKind as Kind, CheckpointRequest, Effect, Event, Frame, Property,
};
use scripted::*;

fn revert(prefix: &str) -> Action {
    Action::RevertBatch {
        actor: "agent:x".into(),
        key_prefix: prefix.into(),
    }
}

fn restore(n: u32) -> Action {
    Action::RestoreCheckpoint { id: n.to_string() }
}

fn x(c: &Cp, id: &str) -> f64 {
    get(&c.l.app, id).x
}

/// The agent moves `r1`; the server has one row before it.
fn after_the_agent() -> (Cp, u32) {
    let mut c = Cp::live();
    let before = c.make(Kind::Manual, Some("before the agent"), None);
    c.l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(50.0)));
    (c, before)
}

#[test]
fn a_revert_asks_for_its_row_first_and_changes_nothing_until_the_server_made_it() {
    let (mut c, _) = after_the_agent();
    c.silent = true;
    let fx = c.send(revert("b1"));
    assert_eq!(
        c.posts(),
        [(Kind::BeforeRevert, Some("agent:x".to_string()), 1)],
        "the file the person sees"
    );
    assert!(sent(&fx).is_empty(), "nothing is sent yet");
    assert_eq!(x(&c, "r1"), 50.0);
    let Some(Effect::PostCheckpoint { key, .. }) = fx.first() else {
        panic!("{fx:?}")
    };
    let row = CheckpointRowFor::before_revert(2, 1);
    let fx = c.l.send(Action::CheckpointPosted {
        key: key.clone(),
        checkpoint: row,
    });
    assert_eq!(x(&c, "r1"), 0.0, "now the revert is applied");
    assert_eq!(sent(&fx).len(), 1);
    assert!(
        c.l.events
            .iter()
            .any(|e| matches!(e, Event::Reverted { requested: 1, .. }))
    );
    assert_eq!(
        c.l.app.state().checkpoints.len(),
        2,
        "the row is in the list"
    );
    c.l.send(Action::Undo {});
    assert_eq!(x(&c, "r1"), 50.0, "one undo takes the revert back");
}

/// The rows of the scripted server, for a test that answers by hand.
struct CheckpointRowFor;
impl CheckpointRowFor {
    fn before_revert(n: u32, seq: u64) -> meldframe_app::CheckpointRow {
        meldframe_app::CheckpointRow {
            n,
            seq,
            kind: Kind::BeforeRevert,
            label: None,
            about: Some("agent:x".into()),
            by: Some("user:me".into()),
            at_ms: 1,
        }
    }
}

#[test]
fn a_revert_with_the_server_playing_along_is_one_step_and_takes_one_row() {
    let (mut c, _) = after_the_agent();
    let fx = c.send(revert("b1"));
    assert_eq!(x(&c, "r1"), 0.0);
    assert_eq!(sent(&fx).len(), 1);
    assert_eq!(c.posts().len(), 1);
    assert_eq!(c.rows.last().unwrap().kind, Kind::BeforeRevert);
    c.l.send(Action::Undo {});
    assert_eq!(x(&c, "r1"), 50.0);
}

#[test]
fn a_failed_post_refuses_the_revert_and_sends_nothing_and_the_person_may_try_again() {
    let (mut c, _) = after_the_agent();
    c.fail_post = Some("unavailable");
    let fx = c.send(revert("b1"));
    assert!(sent(&fx).is_empty());
    assert_eq!(refused(&mut c.l), vec!["checkpoint_failed"]);
    assert_eq!(x(&c, "r1"), 50.0);
    assert!(c.l.app.state().pending.is_empty());
    c.fail_post = None;
    let fx = c.send(revert("b1"));
    assert_eq!((x(&c, "r1"), sent(&fx).len()), (0.0, 1));
    let keys: Vec<_> = c
        .asked
        .iter()
        .filter_map(|e| match e {
            Effect::PostCheckpoint { key, .. } => Some(key.clone()),
            _ => None,
        })
        .collect();
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn no_answer_in_time_is_a_failure_and_a_second_request_waits_its_turn() {
    let (mut c, _) = after_the_agent();
    c.silent = true;
    let fx = c.send(revert("b1"));
    let Some(Effect::PostCheckpoint { key, .. }) = fx.first() else {
        panic!("{fx:?}")
    };
    c.send(revert("b1"));
    c.send(restore(1));
    assert_eq!(
        refused(&mut c.l),
        vec!["checkpoint_pending", "checkpoint_pending"]
    );
    // The shell gives up on the post (its client timeout) and tells the app.
    let request = CheckpointRequest::Post { key: key.clone() };
    c.l.send(Action::CheckpointRequestFailed {
        request,
        code: "timeout".into(),
    });
    assert_eq!(refused(&mut c.l), vec!["checkpoint_failed"]);
    assert_eq!(x(&c, "r1"), 50.0);
    c.silent = false;
    c.send(revert("b1"));
    assert_eq!(x(&c, "r1"), 0.0, "the slot is free again");
}

#[test]
fn where_the_caller_may_not_make_a_checkpoint_the_revert_goes_ahead_without_one() {
    let mut c = Cp::live();
    c.l.send(Action::CheckpointsLoaded {
        checkpoints: vec![],
        can_create: false,
    });
    c.l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(50.0)));
    let fx = c.send(revert("b1"));
    assert_eq!((x(&c, "r1"), sent(&fx).len()), (0.0, 1));
    assert!(c.posts().is_empty());
}

#[test]
fn a_revert_that_is_refused_takes_no_row() {
    let (mut c, _) = after_the_agent();
    c.send(revert("nope"));
    c.l.app.set_max_pending(0);
    c.send(revert("b1"));
    assert_eq!(refused(&mut c.l), vec!["unknown_batch", "outbox_full"]);
    assert!(c.posts().is_empty());
}

#[test]
fn a_stale_answer_changes_nothing_but_the_list() {
    let (mut c, _) = after_the_agent();
    c.silent = true;
    c.send(revert("b1"));
    let row = CheckpointRowFor::before_revert(2, 1);
    c.l.send(Action::CheckpointPosted {
        key: "not-mine".into(),
        checkpoint: row,
    });
    assert_eq!(x(&c, "r1"), 50.0, "it is not the row the revert waits for");
    assert_eq!(c.l.app.state().checkpoints.len(), 2);
    c.l.send(Action::CheckpointStateLoaded {
        n: 1,
        seq: 1,
        nodes: vec![],
    });
    assert!(refused(&mut c.l).is_empty());
}

#[test]
fn a_file_at_another_seq_than_the_checkpoint_has_is_refused_and_asks_for_no_row() {
    let (mut c, before) = after_the_agent();
    c.silent = true;
    c.send(restore(before));
    let nodes = c.state_of(before).1;
    c.l.send(Action::CheckpointStateLoaded {
        n: before,
        seq: 99,
        nodes,
    });
    assert_eq!(refused(&mut c.l), vec!["log_mismatch"]);
    assert!(c.posts().is_empty());
    assert_eq!(x(&c, "r1"), 50.0);
}

#[test]
fn a_restore_reads_the_file_at_the_checkpoint_asks_for_its_row_and_appends_the_change() {
    let (mut c, before) = after_the_agent();
    c.l.send(Action::SetProperty {
        id: "t1".into(),
        property: Property::Name("Headline".into()),
    });
    let head = c.l.seq;
    let fx = c.send(restore(before));
    assert!(matches!(&fx[0], Effect::FetchCheckpointState { n } if *n == before));
    assert_eq!(
        c.posts(),
        [(Kind::BeforeRestore, None, head)],
        "a row at the file the person sees"
    );
    assert_eq!((x(&c, "r1"), get(&c.l.app, "t1").name.as_str()), (0.0, ""));
    assert_eq!(sent(&fx).len(), 2, "two commands, appended to the log");
    assert!(c.l.seq > head, "the log only grew");
    assert!(
        c.l.events
            .iter()
            .any(|e| matches!(e, Event::Restored { requested: 2, .. }))
    );
    c.l.send(Action::Undo {});
    assert_eq!(
        (x(&c, "r1"), get(&c.l.app, "t1").name.as_str()),
        (50.0, "Headline")
    );
}

#[test]
fn a_restore_still_works_after_the_page_loaded_the_file_again() {
    let (mut c, before) = after_the_agent();
    let nodes = c.l.app.nodes().to_vec();
    let reload = Action::Reload {
        now_ms: 0,
        nodes,
        seq: c.l.seq,
        applied: vec![],
        variables: Default::default(),
    };
    c.l.app.dispatch(reload);
    c.l.app.dispatch(Action::Connected { now_ms: 1 });
    let fx = c.send(restore(before));
    assert_eq!((x(&c, "r1"), sent(&fx).len()), (0.0, 1));
}

#[test]
fn a_restore_with_nothing_to_change_asks_for_no_row() {
    let mut c = Cp::live();
    let now = c.make(Kind::Manual, Some("now"), None);
    c.send(restore(now));
    assert_eq!(refused(&mut c.l), vec!["nothing_to_restore"]);
    assert!(c.posts().is_empty());
}

#[test]
fn a_restore_the_server_cannot_read_or_the_person_may_not_is_refused_with_its_code() {
    let (mut c, before) = after_the_agent();
    c.fail_state = Some("too_far");
    c.send(restore(before));
    c.send(restore(99));
    assert_eq!(refused(&mut c.l), vec!["too_far", "unknown_checkpoint"]);
    assert_eq!(x(&c, "r1"), 50.0);
    c.fail_state = None;
    c.send(restore(before));
    assert_eq!(x(&c, "r1"), 0.0, "the failure left the slot free");
    let mut l = Loopback::live();
    l.send(Action::CheckpointsLoaded {
        checkpoints: vec![meldframe_app::CheckpointRow {
            n: 1,
            seq: 0,
            kind: Kind::Manual,
            label: None,
            about: None,
            by: Some("user:me".into()),
            at_ms: 1,
        }],
        can_create: false,
    });
    l.send(restore(1));
    assert_eq!(refused(&mut l), vec!["forbidden"]);
}

#[test]
fn a_restore_that_does_not_fit_the_queue_is_refused_whole_and_asks_for_no_row() {
    let (mut c, before) = after_the_agent();
    c.l.remote("agent:x", "b1:1", 0, set("t1", |p| p.x = Some(10.0)));
    c.l.app.set_max_pending(1);
    c.send(restore(before));
    assert_eq!(refused(&mut c.l), vec!["outbox_full"]);
    assert_eq!((x(&c, "r1"), c.l.app.state().pending.len()), (50.0, 0));
    assert!(c.posts().is_empty());
}

#[test]
fn a_file_loaded_again_while_a_row_is_awaited_refuses_what_waited() {
    let (mut c, _) = after_the_agent();
    c.silent = true;
    c.send(revert("b1"));
    let nodes = c.l.app.nodes().to_vec();
    let reload = Action::Reload {
        now_ms: 0,
        nodes,
        seq: c.l.seq,
        applied: vec![],
        variables: Default::default(),
    };
    c.l.app.dispatch(reload);
    c.l.events.extend(c.l.app.take_events());
    assert_eq!(refused(&mut c.l), vec!["checkpoint_failed"]);
}

#[test]
fn a_restore_across_an_added_frame_leaves_it_empty_and_names_the_refused_removal() {
    let mut c = Cp::live();
    let before = c.make(Kind::Manual, Some("before"), None);
    c.l.remote("agent:x", "b1:0", 0, add("hero", "frame", "root"));
    c.l.remote("agent:x", "b1:1", 0, add("pic", "rect", "hero"));
    // The server confirms the leaf and refuses the frame, as the Delete tier does, so the loopback is not used for it.
    let effects = c.l.app.dispatch(restore(before));
    let state_asked = effects
        .iter()
        .any(|e| matches!(e, Effect::FetchCheckpointState { .. }));
    assert!(state_asked);
    let (seq, nodes) = c.state_of(before);
    let effects = c.l.app.dispatch(Action::CheckpointStateLoaded {
        n: before,
        seq,
        nodes,
    });
    let Some(Effect::PostCheckpoint { key, .. }) = effects.first().cloned() else {
        panic!("{effects:?}")
    };
    let row = meldframe_app::CheckpointRow {
        n: 2,
        seq: c.l.seq,
        kind: Kind::BeforeRestore,
        label: None,
        about: None,
        by: Some("user:me".into()),
        at_ms: 1,
    };
    let effects = c.l.app.dispatch(Action::CheckpointPosted {
        key,
        checkpoint: row,
    });
    let ops: Vec<_> = effects
        .iter()
        .flat_map(|e| match e {
            Effect::SendOps { ops } => ops.clone(),
            _ => vec![],
        })
        .collect();
    assert_eq!(ops.len(), 2, "remove pic, then remove hero");
    let seq = c.l.seq + 1;
    let confirmed = Frame::Event {
        seq,
        actor: "user:me".into(),
        key: Some(ops[0].key.clone()),
        command: ops[0].command.clone(),
    };
    c.l.app.dispatch(Action::Frame {
        now_ms: 0,
        frame: confirmed,
    });
    let refused_frame = Frame::Rejected {
        key: ops[1].key.clone(),
        code: "approval_required".into(),
        retryable: false,
    };
    c.l.app.dispatch(Action::Frame {
        now_ms: 0,
        frame: refused_frame,
    });
    c.l.events.extend(c.l.app.take_events());
    let nodes = c.l.app.nodes();
    assert!(nodes.iter().any(|n| n.id == "hero") && !nodes.iter().any(|n| n.id == "pic"));
    assert!(c.l.app.state().pending.is_empty());
    let rejected: Vec<_> =
        c.l.events
            .iter()
            .filter_map(|e| match e {
                Event::OpRejected { node, code, .. } => Some((node.clone(), code.clone())),
                _ => None,
            })
            .collect();
    assert_eq!(
        rejected,
        vec![(Some("hero".to_string()), "approval_required".to_string())]
    );
    assert!(
        c.l.events
            .iter()
            .any(|e| matches!(e, Event::Restored { requested: 2, .. }))
    );
}
