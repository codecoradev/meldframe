//! E5.1 at the app seam (ADR-0021): batches, notes, review states, lanes, touch rings and the fetches that heal a lost
//! frame, all through `App::dispatch`. The reducer's own rules are tested in `meldframe-activity-tests`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Effect, Event, Frame};
use meldframe_app_model::{
    ActivityState, BatchHeader, BatchPage, BatchRef, BatchStatus, LaneState, ReviewState,
};
use meldframe_protocol::{Command, NodePatch};

/// An epoch time, as the server's `atMs` and the front end's clock have.
const T: u64 = 1_700_000_000_000;

fn header(n: u32, rev: u64, actor: &str, prefix: &str, written: u32) -> BatchHeader {
    BatchHeader {
        n,
        rev,
        actor: actor.into(),
        key_prefix: prefix.into(),
        tool: "apply_commands".into(),
        commands: written,
        written,
        atomic: false,
        first_seq: 1,
        last_seq: written as u64,
        at_ms: T,
        note: None,
        review: ReviewState::Unreviewed,
        reviewed_by: None,
        reviewed_at_ms: None,
    }
}

fn batch(h: BatchHeader, count: u32, now_ms: u64) -> Action {
    let frame = Frame::Batch {
        header: h,
        unreviewed_count: count,
    };
    Action::Frame { now_ms, frame }
}

fn page(batches: Vec<BatchHeader>, has_more: bool, count: u32) -> Action {
    let page = BatchPage {
        batches,
        has_more,
        unreviewed_count: count,
        can_review: true,
    };
    Action::BatchesLoaded { page }
}

fn paint(id: &str, x: f64) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            x: Some(x),
            ..Default::default()
        },
    }
}

fn add(id: &str, parent: &str) -> Command {
    Command::AddNode {
        node: node(id, "frame", Some(parent)),
        index: None,
    }
}

fn fetches(fx: &[Effect]) -> Vec<(Option<u32>, Option<u32>, u32)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::FetchBatches {
                before_n,
                after_n,
                limit,
            } => Some((*before_n, *after_n, *limit)),
            _ => None,
        })
        .collect()
}

fn refused(l: &mut Loopback) -> Vec<String> {
    let all = l.app.take_events();
    l.events.extend(all);
    l.events
        .iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code.clone()),
            _ => None,
        })
        .collect()
}

fn act(l: &Loopback) -> &ActivityState {
    &l.app.state().activity
}

fn by(actor: &str, prefix: &str) -> BatchRef {
    BatchRef {
        actor: actor.into(),
        key_prefix: prefix.into(),
    }
}

/// The agent moves `r1` and `t1` as batch `b1` (two ops) and its header arrives with a note.
fn agent_batch() -> Loopback {
    let mut l = Loopback::live();
    l.remote("agent:x", "b1:0", T, paint("r1", 50.0));
    l.remote("agent:x", "b1:1", T, paint("t1", 20.0));
    let mut h = header(1, 1, "agent:x", "b1", 2);
    h.note = Some("Moved the cards".into());
    l.app.dispatch(batch(h, 1, T));
    l.events.extend(l.app.take_events());
    l
}

#[test]
fn a_batch_of_an_agent_is_one_row_with_its_note_and_a_working_lane() {
    let l = agent_batch();
    let a = act(&l);
    let row = a.row(&by("agent:x", "b1")).expect("a row");
    assert_eq!(
        (row.n, row.status, row.note.as_deref()),
        (Some(1), BatchStatus::Applied, Some("Moved the cards"))
    );
    assert_eq!((row.can_show, row.revertable), (true, true));
    assert_eq!(row.summary.as_ref().map(|s| s.changed), Some(2));
    assert_eq!(a.unreviewed_count, Some(1));
    assert_eq!(a.lanes[0].actor, "agent:x");
    assert_eq!(a.lanes[0].state, LaneState::Working);
    assert!(l.events.contains(&Event::ActivityChanged {}));
}

#[test]
fn a_note_is_cleaned_again_in_the_app() {
    let mut l = Loopback::live();
    let mut h = header(1, 1, "agent:x", "b1", 1);
    h.note = Some("a\u{200b}b\u{202e}c".into());
    l.app.dispatch(batch(h, 1, T));
    assert_eq!(act(&l).rows[0].note.as_deref(), Some("abc"));
}

#[test]
fn ops_with_no_header_wait_then_ask_three_times_then_stay_headerless() {
    let mut l = Loopback::live();
    l.remote("agent:x", "b1:0", T, paint("r1", 5.0));
    let row = |l: &Loopback| act(l).rows[0].status;
    assert_eq!(row(&l), BatchStatus::Building);
    let tick = |l: &mut Loopback, at| l.app.dispatch(Action::Tick { now_ms: T + at });
    assert!(fetches(&tick(&mut l, 2_000)).is_empty(), "not yet");
    assert_eq!(fetches(&tick(&mut l, 3_000)), vec![(None, None, 100)]);
    assert!(fetches(&tick(&mut l, 4_000)).is_empty(), "one in 5 seconds");
    assert_eq!(fetches(&tick(&mut l, 8_000)).len(), 1);
    assert_eq!(fetches(&tick(&mut l, 13_000)).len(), 1);
    assert_eq!(
        row(&l),
        BatchStatus::Headerless,
        "the third try was the last"
    );
    assert!(fetches(&tick(&mut l, 20_000)).is_empty());
    l.app
        .dispatch(batch(header(1, 1, "agent:x", "b1", 1), 1, T + 21_000));
    assert_eq!(row(&l), BatchStatus::Applied, "a header ends it");
}

#[test]
fn a_gap_in_rev_asks_for_the_newest_page() {
    let mut l = Loopback::live();
    let fx = l
        .app
        .dispatch(batch(header(1, 1, "agent:x", "b1", 1), 1, T));
    assert!(fetches(&fx).is_empty());
    let fx = l
        .app
        .dispatch(batch(header(3, 3, "agent:x", "b3", 1), 2, T));
    assert_eq!(fetches(&fx), vec![(None, None, 100)]);
    assert_eq!(act(&l).unreviewed_count, Some(2));
}

#[test]
fn opening_the_tab_and_a_new_subscription_read_down_to_the_lowest_batch_held() {
    let mut l = Loopback::live();
    let fx = l.app.dispatch(Action::OpenActivity {});
    assert_eq!(fetches(&fx), vec![(None, None, 100)]);
    // Held: 5 to 7. A resync reads the newest page and goes down while pages are full and the floor is not reached.
    l.app.dispatch(page(
        vec![header(5, 5, "a:1", "p5", 1), header(7, 7, "a:1", "p7", 1)],
        false,
        2,
    ));
    let marker = Frame::Resync {
        after_seq: 0,
        latest_seq: 0,
    };
    let fx = l.app.dispatch(Action::Frame {
        now_ms: T,
        frame: marker,
    });
    assert_eq!(fetches(&fx), vec![(None, None, 100)]);
    let fx = l.app.dispatch(page(
        vec![header(9, 9, "a:1", "p9", 1), header(7, 7, "a:1", "p7", 1)],
        true,
        4,
    ));
    assert_eq!(fetches(&fx), vec![(Some(7), None, 100)]);
    let fx = l
        .app
        .dispatch(page(vec![header(4, 4, "a:1", "p4", 1)], true, 4));
    assert!(
        fetches(&fx).is_empty(),
        "page reached the floor, the chain ends"
    );
}

#[test]
fn the_heal_chain_stops_after_five_pages_and_on_a_failure() {
    let mut l = Loopback::live();
    let mut asked = 1; // the first page, asked for by `Connected` when nothing is held
    let mut low = 1_000;
    loop {
        low -= 100;
        let fx = l.app.dispatch(page(
            vec![header(low, low as u64, "a:1", &format!("p{low}"), 1)],
            true,
            1,
        ));
        if fetches(&fx).is_empty() {
            break;
        }
        asked += 1;
    }
    assert_eq!(asked, 5);
    let mut l = Loopback::live();
    l.app.dispatch(Action::BatchRequestFailed {
        code: "unavailable".into(),
    });
    assert_eq!(refused(&mut l), vec!["unavailable"]);
}

#[test]
fn load_older_asks_below_the_lowest_batch_held_and_only_when_there_is_one() {
    let mut l = Loopback::live();
    assert!(fetches(&l.app.dispatch(Action::LoadOlderBatches {})).is_empty());
    l.app
        .dispatch(page(vec![header(5, 5, "a:1", "p5", 1)], true, 1));
    assert!(act(&l).has_older);
    let fx = l.app.dispatch(Action::LoadOlderBatches {});
    assert_eq!(fetches(&fx), vec![(Some(5), None, 100)]);
    l.app
        .dispatch(page(vec![header(1, 1, "a:1", "p1", 1)], false, 1));
    assert!(!act(&l).has_older);
}

#[test]
fn a_review_changes_the_row_only_when_the_server_says_so() {
    let mut l = agent_batch();
    let put = |l: &mut Loopback, state| {
        let batch = by("agent:x", "b1");
        l.app.dispatch(Action::SetBatchReview { batch, state })
    };
    let fx = put(&mut l, ReviewState::Accepted);
    assert!(matches!(
        &fx[..],
        [Effect::PutBatchReview {
            n: 1,
            state: ReviewState::Accepted
        }]
    ));
    let row = |l: &Loopback| act(l).row(&by("agent:x", "b1")).unwrap().review;
    assert_eq!(row(&l), ReviewState::Unreviewed, "not at once");
    // The server refuses: the row stays as it was, and the refusal is raised.
    l.app.dispatch(Action::BatchRequestFailed {
        code: "forbidden".into(),
    });
    assert_eq!(refused(&mut l), vec!["forbidden"]);
    assert_eq!(
        (row(&l), act(&l).unreviewed_count),
        (ReviewState::Unreviewed, Some(1))
    );
    // The answer is the row after the change; the frame of an older rev changes nothing.
    let mut h = header(1, 2, "agent:x", "b1", 2);
    (h.review, h.reviewed_by, h.reviewed_at_ms) =
        (ReviewState::Accepted, Some("user:me".into()), Some(T));
    l.app.dispatch(Action::BatchReviewed { batch: h.clone() });
    assert_eq!(
        (row(&l), act(&l).unreviewed_count),
        (ReviewState::Accepted, Some(1))
    );
    l.app.dispatch(batch(h, 0, T)); // the frame of the change brings the count
    assert_eq!(
        (row(&l), act(&l).unreviewed_count),
        (ReviewState::Accepted, Some(0))
    );
    l.app
        .dispatch(batch(header(1, 1, "agent:x", "b1", 2), 1, T));
    assert_eq!(
        (row(&l), act(&l).unreviewed_count),
        (ReviewState::Accepted, Some(0))
    );
}

#[test]
fn a_review_of_a_row_that_is_not_there_or_has_no_number_is_refused() {
    let mut l = Loopback::live();
    l.remote("agent:x", "b1:0", T, paint("r1", 5.0));
    let ask = |l: &mut Loopback, prefix| {
        let batch = by("agent:x", prefix);
        let fx = l.app.dispatch(Action::SetBatchReview {
            batch,
            state: ReviewState::Accepted,
        });
        (
            fx.iter()
                .any(|e| matches!(e, Effect::PutBatchReview { .. })),
            refused(l),
        )
    };
    assert_eq!(
        ask(&mut l, "b1"),
        (false, vec!["no_batch_number".to_string()])
    );
    assert_eq!(
        ask(&mut l, "nope"),
        (
            false,
            vec!["no_batch_number".into(), "unknown_batch".into()]
        )
    );
}

#[test]
fn a_revert_does_not_change_the_activity_state_by_itself() {
    let mut l = agent_batch();
    l.app.dispatch(Action::SelectBatch {
        batch: by("agent:x", "b1"),
    });
    assert_eq!(
        act(&l).row(&by("agent:x", "b1")).unwrap().in_effect,
        Some(true)
    );
    let before = act(&l).clone();
    let revert = Action::RevertBatch {
        actor: "agent:x".into(),
        key_prefix: "b1".into(),
    };
    let fx = l.app.dispatch(revert);
    assert!(!sent(&fx).is_empty(), "the revert went out");
    assert_eq!(
        act(&l),
        &before,
        "nothing in the panel moved when the revert was asked for"
    );
    // Once the server confirms the person's ops the row is no longer in effect, and its review and the count stay.
    for e in &fx {
        if let Effect::SendOps { ops } = e {
            for o in ops {
                l.remote("user:me", &o.key, T + 1, o.command.clone());
            }
        }
    }
    let row = act(&l).row(&by("agent:x", "b1")).unwrap();
    assert_eq!(
        (row.in_effect, row.review, act(&l).unreviewed_count),
        (Some(false), ReviewState::Unreviewed, Some(1))
    );
}

#[test]
fn show_selects_the_nodes_of_a_batch_and_an_unknown_batch_is_refused() {
    let mut l = agent_batch();
    l.app.dispatch(Action::SelectBatch {
        batch: by("agent:x", "b1"),
    });
    assert_eq!(l.app.state().selection, ids(&["r1", "t1"]));
    assert_eq!(act(&l).selected, Some(by("agent:x", "b1")));
    l.app.dispatch(Action::SelectBatch {
        batch: by("agent:x", "zz"),
    });
    assert_eq!(refused(&mut l), vec!["unknown_batch"]);
    assert_eq!(act(&l).selected, Some(by("agent:x", "b1")));
}

#[test]
fn touch_rings_come_and_go_and_a_working_lane_keeps_one_on_its_top_level_nodes() {
    let mut l = Loopback::live();
    let touch = |l: &mut Loopback, actor: &str, at| {
        let frame = Frame::Touch {
            actor: actor.into(),
            ids: ids(&["r1"]),
            seq: 1,
        };
        l.app.dispatch(Action::Frame { now_ms: at, frame });
    };
    touch(&mut l, "user:me", T);
    assert!(
        act(&l).touches.is_empty(),
        "no ring for the person's own touches"
    );
    touch(&mut l, "agent:x", T);
    assert_eq!(act(&l).touches.len(), 1);
    l.app.dispatch(Action::Tick { now_ms: T + 1_500 });
    assert!(act(&l).touches.is_empty(), "gone after 1.5 seconds");
    // The agent adds a frame with a child: while it works, the ring covers the top-level node only.
    l.remote("agent:x", "b2:0", T + 2_000, add("hero", "root"));
    l.remote("agent:x", "b2:1", T + 2_000, add("pic", "hero"));
    let ring = act(&l)
        .touches
        .iter()
        .find(|t| t.persistent)
        .cloned()
        .expect("a ring");
    assert_eq!(ring.ids, ids(&["hero"]));
    l.app.dispatch(Action::Tick { now_ms: T + 13_000 });
    assert!(act(&l).touches.is_empty(), "a quiet lane keeps no ring");
}

#[test]
fn a_reload_forgets_the_headers_and_the_next_subscription_reads_them_again() {
    let mut l = agent_batch();
    let reload = Action::Reload {
        now_ms: T,
        nodes: doc(),
        variables: Default::default(),
        seq: l.seq,
        applied: vec![],
    };
    let fx = l.app.dispatch(reload);
    assert!(act(&l).rows.is_empty() && act(&l).unreviewed_count.is_none());
    assert!(fetches(&fx).is_empty(), "not live until it subscribes");
    let fx = l.app.dispatch(Action::Connected { now_ms: T });
    assert_eq!(fetches(&fx), vec![(None, None, 100)]);
}
