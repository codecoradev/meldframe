//! The tab of the right card, the question before a revert or a restore, and whether the checkpoint a person asked for was
//! made are state of the app layer (audit finding T10, issue 323): a front end that has none of them can open the Activity
//! tab, ask to revert a batch and confirm it. The server is scripted (`scripted/mod.rs`).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
mod scripted;
use common::*;
use meldframe_app::{
    Action, ActivityOp, BatchRef, CheckpointKind as Kind, Confirm, Connection, Effect, Event,
    RightTab,
};
use scripted::*;

fn op(op: ActivityOp) -> Action {
    Action::Activity { op }
}

fn batch(prefix: &str) -> BatchRef {
    BatchRef {
        actor: "agent:x".into(),
        key_prefix: prefix.into(),
    }
}

fn x(c: &Cp, id: &str) -> f64 {
    get(&c.l.app, id).x
}

fn confirm(c: &Cp) -> Option<Confirm> {
    c.l.app.state().confirm.clone()
}

fn refused(c: &mut Cp) -> Vec<String> {
    let events = std::mem::take(&mut c.l.events);
    events
        .into_iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect()
}

/// Two batches of the agent: `b1` moves `r1`, `b2` moves `t1`. The server has one row before them.
fn two_batches() -> Cp {
    let mut c = Cp::live();
    c.make(Kind::Manual, Some("before the agent"), None);
    c.l.remote("agent:x", "b1:0", 0, set("r1", |p| p.x = Some(50.0)));
    c.l.remote("agent:x", "b2:0", 0, set("t1", |p| p.x = Some(70.0)));
    c.l.events.clear();
    c
}

#[test]
fn ask_revert_selects_batch_and_opens_confirmation() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    assert_eq!(
        confirm(&c),
        Some(Confirm::Revert {
            actor: "agent:x".into(),
            key_prefix: "b1".into()
        })
    );
    assert_eq!(c.l.app.state().activity.selected, Some(batch("b1")));
    assert_eq!(
        c.l.app.state().selection,
        ["r1"],
        "the batch is shown on the canvas"
    );
    assert_eq!(x(&c, "r1"), 50.0, "nothing is reverted by asking");
}

#[test]
fn ask_revert_for_a_batch_the_app_does_not_hold_is_refused_and_asks_nothing() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRevert {
        batch: batch("nope"),
    }));
    assert_eq!(refused(&mut c), ["unknown_batch"]);
    assert_eq!(confirm(&c), None);
}

#[test]
fn confirm_revert_reverts_once_and_closes_the_question() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    c.send(op(ActivityOp::ConfirmRevert { batch: batch("b1") }));
    assert_eq!(x(&c, "r1"), 0.0);
    assert_eq!(confirm(&c), None);
    assert!(
        c.l.events
            .iter()
            .any(|e| matches!(e, Event::Reverted { requested: 1, .. }))
    );
    // Answered once: a second confirmation has no question to answer.
    let steps = c.l.app.state().history.undo_steps;
    c.send(op(ActivityOp::ConfirmRevert { batch: batch("b1") }));
    assert_eq!(c.l.app.state().history.undo_steps, steps);
    c.l.send(Action::Undo {});
    assert_eq!(x(&c, "r1"), 50.0, "one undo takes the revert back");
}

#[test]
fn showing_another_batch_closes_the_question_so_confirming_it_afterwards_does_nothing() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    // The person shows another batch: the selection moves away from the question, which closes.
    c.send(Action::SelectBatch { batch: batch("b2") });
    assert_eq!(
        confirm(&c),
        None,
        "a revert question for another batch closes"
    );
    c.send(op(ActivityOp::ConfirmRevert { batch: batch("b1") }));
    assert_eq!(x(&c, "r1"), 50.0);
    // And a confirmation with no question at all does nothing.
    c.send(op(ActivityOp::ConfirmRevert { batch: batch("b2") }));
    assert_eq!(x(&c, "t1"), 70.0);
    assert!(c.posts().is_empty(), "no checkpoint was asked for either");
}

#[test]
fn showing_the_batch_the_question_is_about_keeps_it() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    c.send(Action::SelectBatch { batch: batch("b1") });
    assert!(confirm(&c).is_some());
}

#[test]
fn confirmation_closes_when_row_disappears() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    // The file is loaded again from a snapshot that has none of the batches: the row is gone.
    let nodes = c.l.app.base().to_vec();
    c.l.send(Action::Reload {
        now_ms: 5,
        nodes,
        variables: Default::default(),
        seq: c.l.seq,
        applied: Vec::new(),
    });
    assert_eq!(confirm(&c), None);
    c.send(op(ActivityOp::ConfirmRevert { batch: batch("b1") }));
    assert!(c.posts().is_empty());
}

#[test]
fn a_restore_question_is_asked_for_a_checkpoint_that_is_there_and_answered_once() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRestore { id: "9".into() }));
    assert_eq!(refused(&mut c), ["unknown_checkpoint"]);
    assert_eq!(confirm(&c), None);
    c.send(op(ActivityOp::AskRestore { id: "1".into() }));
    assert_eq!(confirm(&c), Some(Confirm::Restore { id: "1".into() }));
    // A confirmation for another checkpoint is not the answer to this question.
    c.send(op(ActivityOp::ConfirmRestore { id: "2".into() }));
    assert_eq!(confirm(&c), Some(Confirm::Restore { id: "1".into() }));
    c.send(op(ActivityOp::ConfirmRestore { id: "1".into() }));
    assert_eq!(confirm(&c), None);
    assert_eq!(x(&c, "r1"), 0.0, "the file is back at the checkpoint");
}

#[test]
fn a_restore_question_closes_when_its_checkpoint_is_gone() {
    let mut c = two_batches();
    c.make(Kind::Manual, Some("second"), None);
    c.send(op(ActivityOp::AskRestore { id: "1".into() }));
    assert_eq!(confirm(&c), Some(Confirm::Restore { id: "1".into() }));
    // The server's list no longer has row 1 (it was rolled out): the question has nothing to ask about.
    c.l.send(Action::CheckpointsLoaded {
        checkpoints: vec![c.rows[1].clone()],
        can_create: true,
    });
    assert_eq!(confirm(&c), None);
}

#[test]
fn cancel_closes_the_question_and_creating_a_checkpoint_closes_it_too() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    c.send(op(ActivityOp::Cancel {}));
    assert_eq!(confirm(&c), None);
    c.send(op(ActivityOp::AskRestore { id: "1".into() }));
    c.send(Action::CreateCheckpoint { label: "v2".into() });
    assert_eq!(confirm(&c), None, "the person moved on");
    // A refused creation moves nobody on: the question stays.
    c.send(op(ActivityOp::AskRestore { id: "1".into() }));
    c.send(Action::CreateCheckpoint {
        label: "bad\u{202e}".into(),
    });
    assert_eq!(confirm(&c), Some(Confirm::Restore { id: "1".into() }));
}

#[test]
fn a_question_is_one_at_a_time_and_a_change_of_it_is_an_event() {
    let mut c = two_batches();
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    c.l.events.clear();
    c.send(op(ActivityOp::AskRestore { id: "1".into() }));
    assert_eq!(confirm(&c), Some(Confirm::Restore { id: "1".into() }));
    assert!(c.l.events.contains(&Event::ConfirmChanged {}));
    c.l.events.clear();
    c.send(op(ActivityOp::AskRestore { id: "1".into() }));
    assert!(
        !c.l.events.contains(&Event::ConfirmChanged {}),
        "the same question again changes nothing"
    );
}

#[test]
fn checkpoint_created_event_names_the_request_key() {
    let mut c = Cp::live();
    c.l.events.clear();
    c.send(Action::CreateCheckpoint { label: "v1".into() });
    let key = c.asked.iter().find_map(|e| match e {
        Effect::PostCheckpoint { key, .. } => Some(key.clone()),
        _ => None,
    });
    let made: Vec<_> =
        c.l.events
            .iter()
            .filter_map(|e| match e {
                Event::CheckpointMade { key } => Some(key.clone()),
                _ => None,
            })
            .collect();
    assert_eq!(made, key.into_iter().collect::<Vec<_>>());
    assert_eq!(made.len(), 1);
}

#[test]
fn a_row_nobody_asked_for_and_a_refused_request_do_not_say_it_was_made() {
    let mut c = Cp::live();
    c.l.events.clear();
    // An agent's run start and another person's row arrive by frames.
    c.make(Kind::RunStart, None, Some("agent:x"));
    c.make(Kind::Manual, Some("theirs"), None);
    // The person's own request is refused by the server.
    c.fail_post = Some("checkpoint_full");
    c.send(Action::CreateCheckpoint {
        label: "mine".into(),
    });
    assert!(
        !c.l.events
            .iter()
            .any(|e| matches!(e, Event::CheckpointMade { .. }))
    );
    // The same text asked for again, and made, is a made request.
    c.fail_post = None;
    c.send(Action::CreateCheckpoint {
        label: "mine".into(),
    });
    assert_eq!(
        c.l.events
            .iter()
            .filter(|e| matches!(e, Event::CheckpointMade { .. }))
            .count(),
        1
    );
}

fn tab(c: &mut Cp, tab: RightTab) {
    c.send(op(ActivityOp::SetTab { tab }));
}

#[test]
fn changing_the_tab_closes_the_question_and_coming_back_does_not_bring_it_back() {
    let mut c = two_batches();
    tab(&mut c, RightTab::Activity);
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    assert!(confirm(&c).is_some());
    // The tab it is shown in is already selected: nothing changes, the question stays.
    tab(&mut c, RightTab::Activity);
    assert!(
        confirm(&c).is_some(),
        "a tab that is already selected closes nothing"
    );
    c.l.events.clear();
    tab(&mut c, RightTab::Design);
    assert_eq!(confirm(&c), None, "its screen is gone, so it is denied");
    assert!(c.l.events.contains(&Event::ConfirmChanged {}));
    tab(&mut c, RightTab::Activity);
    assert_eq!(confirm(&c), None, "coming back does not ask again");
    // A confirmation that arrives for the question that was closed has nothing to answer: nothing is reverted.
    c.send(op(ActivityOp::ConfirmRevert { batch: batch("b1") }));
    assert_eq!(x(&c, "r1"), 50.0);
    assert!(c.posts().is_empty(), "and no checkpoint was asked for");
}

#[test]
fn changing_the_tab_closes_a_question_about_a_checkpoint_too() {
    let mut c = two_batches();
    tab(&mut c, RightTab::Activity);
    c.send(op(ActivityOp::AskRestore { id: "1".into() }));
    assert_eq!(confirm(&c), Some(Confirm::Restore { id: "1".into() }));
    tab(&mut c, RightTab::Design);
    tab(&mut c, RightTab::Activity);
    assert_eq!(confirm(&c), None);
    c.send(op(ActivityOp::ConfirmRestore { id: "1".into() }));
    assert!(
        c.posts().is_empty(),
        "nothing was restored: no checkpoint before it either"
    );
}

#[test]
fn a_halted_app_changes_the_tab_but_asks_nothing_and_no_question_is_opened() {
    let mut c = two_batches();
    // An op the file cannot take diverges the replica: the app halts and takes only what changes the view.
    c.l.remote("agent:x", "b3:0", 0, set("ghost", |p| p.x = Some(1.0)));
    assert_eq!(c.l.app.state().connection, Connection::Halted);
    c.l.events.clear();
    let fx = c.send(op(ActivityOp::SetTab {
        tab: RightTab::Activity,
    }));
    assert_eq!(
        c.l.app.state().right_tab,
        RightTab::Activity,
        "the tab is a view"
    );
    assert!(fx.is_empty(), "and asks for nothing while halted");
    c.send(op(ActivityOp::AskRevert { batch: batch("b1") }));
    assert_eq!(confirm(&c), None, "no question is opened by a halted app");
    assert_eq!(c.l.app.state().activity.selected, None);
}

#[test]
fn only_the_last_eight_requests_are_remembered_while_their_answers_are_out() {
    let mut c = Cp::live();
    c.silent = true;
    c.l.events.clear();
    for i in 0..9 {
        c.send(Action::CreateCheckpoint {
            label: format!("v{i}"),
        });
    }
    let keys: Vec<String> = c
        .asked
        .iter()
        .filter_map(|e| match e {
            Effect::PostCheckpoint { key, .. } => Some(key.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(keys.len(), 9);
    let answer = |c: &mut Cp, n: usize| {
        c.l.events.clear();
        let checkpoint = meldframe_app::CheckpointRow {
            n: n as u32 + 1,
            seq: 0,
            kind: Kind::Manual,
            label: Some(format!("v{n}")),
            about: None,
            by: Some("user:me".into()),
            at_ms: 1,
        };
        c.l.send(Action::CheckpointPosted {
            key: keys[n].clone(),
            checkpoint,
        });
        c.l.events
            .iter()
            .filter(|e| matches!(e, Event::CheckpointMade { .. }))
            .count()
    };
    assert_eq!(
        answer(&mut c, 0),
        0,
        "the oldest was forgotten, as the ninth came"
    );
    assert_eq!(answer(&mut c, 1), 1, "the others are remembered");
    assert_eq!(answer(&mut c, 8), 1);
}

#[test]
fn a_manual_row_with_the_same_name_by_someone_else_is_not_my_request() {
    let mut c = Cp::live();
    c.silent = true;
    c.l.events.clear();
    c.send(Action::CreateCheckpoint {
        label: "same".into(),
    });
    // Another person makes a row with the same name while the server has not answered the request yet.
    c.silent = false;
    c.make(Kind::Manual, Some("same"), None);
    assert!(
        !c.l.events
            .iter()
            .any(|e| matches!(e, Event::CheckpointMade { .. })),
        "the old rule guessed by the label"
    );
}
