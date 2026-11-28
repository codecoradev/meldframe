//! Two editors and an agent on one file: the agent writes one batch, both editors show the same row, one accepts it,
//! the other reverts it, and the review state and the count do not move with the revert.
//!
//! Real: `meldframe_server::Server` (gate, room and log, with a test tool `paint`) takes the agent's ops and the
//! editors' ops, and numbers them; `meldframe-app` runs in-process. Scripted: the batch header, the `batch` frames to
//! every stream and the answers of the review route, which the batch sink and routes of `meldframe-http-activity`
//! would give (they are tested against the real server in `apps/server/tests`, not here).

use meldframe_app::{Action, Effect, Frame};
use meldframe_app_harness as support;
use meldframe_app_model::{
    ActivityState, BatchHeader, BatchPage, BatchRef, BatchStatus, ReviewState,
};
use meldframe_host::{Op, Principal};
use meldframe_protocol::{Command, NodePatch};
use support::*;

const T: u64 = 1_700_000_000_000;

fn paint(id: &str, x: f64) -> Command {
    let patch = NodePatch {
        x: Some(x),
        ..Default::default()
    };
    Command::UpdateNode {
        id: id.into(),
        patch,
    }
}

fn header(rev: u64, review: ReviewState, by: Option<&str>) -> BatchHeader {
    BatchHeader {
        n: 1,
        rev,
        actor: "agent:claude".into(),
        key_prefix: "b1".into(),
        tool: "apply_commands".into(),
        commands: 2,
        written: 2,
        atomic: false,
        first_seq: 3,
        last_seq: 4,
        at_ms: T,
        note: Some("Moved the page and its text".into()),
        review,
        reviewed_by: by.map(Into::into),
        reviewed_at_ms: by.map(|_| T),
    }
}

/// What the server queues to every open stream when a header is written or changed.
fn frame_to_all(w: &mut World, h: BatchHeader, count: u32) {
    for i in 0..w.clients.len() {
        let frame = Frame::Batch {
            header: h.clone(),
            unreviewed_count: count,
        };
        w.act(
            i,
            Action::Frame {
                now_ms: w.now,
                frame,
            },
        );
    }
}

fn panel(w: &World, i: usize) -> &ActivityState {
    &w.app(i).state().activity
}

fn me() -> BatchRef {
    BatchRef {
        actor: "agent:claude".into(),
        key_prefix: "b1".into(),
    }
}

#[test]
fn two_editors_see_the_same_batch_and_a_review_and_a_revert_stay_apart() {
    let mut w = World::new(64);
    w.now = T;
    let (alice, bob) = (w.join("user:alice", "a", 1), w.join("user:bob", "b", 1));
    // The agent writes one batch of two ops under keys `b1:0` and `b1:1`, as `apply_commands` does.
    let agent = Principal::new(WS, "agent:claude");
    for (i, c) in [paint("t1", 50.0), paint("root", 10.0)]
        .into_iter()
        .enumerate()
    {
        let op = Op::new(format!("b1:{i}"), c);
        w.server
            .submit_tool("paint", FILE, &agent, vec![op])
            .unwrap();
    }
    w.pump_all();
    frame_to_all(&mut w, header(1, ReviewState::Unreviewed, None), 1);
    for i in [alice, bob] {
        let row = panel(&w, i).row(&me()).expect("both editors have the row");
        assert_eq!(
            (row.n, row.status, row.revertable),
            (Some(1), BatchStatus::Applied, true)
        );
        assert_eq!(row.note.as_deref(), Some("Moved the page and its text"));
        assert_eq!(
            (
                panel(&w, i).unreviewed_count,
                panel(&w, i).lanes[0].actor.as_str()
            ),
            (Some(1), "agent:claude")
        );
    }
    // Alice asks for Accepted; nothing changes until the server answers, and then both editors move together.
    w.act(alice, Action::SelectBatch { batch: me() });
    w.act(
        alice,
        Action::SetBatchReview {
            batch: me(),
            state: ReviewState::Accepted,
        },
    );
    let put = w.clients[alice]
        .effects
        .iter()
        .filter(|e| {
            matches!(
                e,
                Effect::PutBatchReview {
                    n: 1,
                    state: ReviewState::Accepted
                }
            )
        })
        .count();
    assert_eq!(put, 1);
    assert_eq!(
        panel(&w, bob).row(&me()).unwrap().review,
        ReviewState::Unreviewed
    );
    let accepted = header(2, ReviewState::Accepted, Some("user:alice"));
    let answer = BatchPage {
        batches: vec![accepted.clone()],
        has_more: false,
        unreviewed_count: 0,
        can_review: true,
    };
    w.act(alice, Action::BatchesLoaded { page: answer });
    frame_to_all(&mut w, accepted, 0);
    for i in [alice, bob] {
        let row = panel(&w, i).row(&me()).unwrap();
        assert_eq!(
            (row.review, row.reviewed_by.as_deref()),
            (ReviewState::Accepted, Some("user:alice"))
        );
        assert_eq!(panel(&w, i).unreviewed_count, Some(0));
    }
    // Bob reverts the batch: his ops go through the real gate and room and come back to both editors.
    w.act(
        bob,
        Action::RevertBatch {
            actor: "agent:claude".into(),
            key_prefix: "b1".into(),
        },
    );
    w.pump_all();
    w.assert_converged();
    assert_eq!(
        w.app(bob).nodes().iter().find(|n| n.id == "t1").unwrap().x,
        0.0
    );
    let (a, b) = (panel(&w, alice), panel(&w, bob));
    assert_eq!(
        a.row(&me()).unwrap().in_effect,
        Some(false),
        "alice's selected row is no longer in effect"
    );
    for p in [a, b] {
        let row = p.row(&me()).unwrap();
        assert_eq!(
            (row.review, p.unreviewed_count),
            (ReviewState::Accepted, Some(0)),
            "a revert is not a review"
        );
        assert!(
            p.lanes.iter().any(|l| l.actor == "user:bob"),
            "bob's changes are a lane of their own"
        );
    }
}
