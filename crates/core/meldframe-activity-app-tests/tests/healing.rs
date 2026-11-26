//! The activity panel's counts, heal rules and memo under the cases the first review found: batches with nothing to
//! show, an answer of the review route that races a frame, a clock that runs while the connection is down, an answer
//! that is not the one rule (d) waits for, and a reload at the same number. One editor on the harness of
//! `meldframe-app-tests`; where a test says so the ops go through the real `meldframe_server::Server` (gate, room, log),
//! otherwise (and always for headers, batch frames and route answers) they are scripted frames.

use meldframe_app::{Action, Effect, Frame};
use meldframe_app_harness as support;
use meldframe_app_model::{BatchHeader, BatchPage, BatchRef, BatchStatus, ReviewState};
use meldframe_host::{Op, Principal};
use meldframe_protocol::Command;
use serde_json::json;
use support::*;

const T: u64 = 1_700_000_000_000;

fn header(n: u32, rev: u64, actor: &str, prefix: &str) -> BatchHeader {
    BatchHeader {
        n,
        rev,
        actor: actor.into(),
        key_prefix: prefix.into(),
        tool: "apply_commands".into(),
        commands: 1,
        written: 1,
        atomic: false,
        first_seq: 1,
        last_seq: 1,
        at_ms: T,
        note: None,
        review: ReviewState::Unreviewed,
        reviewed_by: None,
        reviewed_at_ms: None,
    }
}

fn page(batches: Vec<BatchHeader>, more: bool, count: u32) -> Action {
    let page = BatchPage {
        batches,
        has_more: more,
        unreviewed_count: count,
        can_review: true,
    };
    Action::BatchesLoaded { page }
}

fn frame(h: BatchHeader, count: u32, now_ms: u64) -> Action {
    Action::Frame {
        now_ms,
        frame: Frame::Batch {
            header: h,
            unreviewed_count: count,
        },
    }
}

fn by(actor: &str, prefix: &str) -> BatchRef {
    BatchRef {
        actor: actor.into(),
        key_prefix: prefix.into(),
    }
}

/// The agent writes ops through the real server, one op at a time under `{prefix}:{i}`.
fn agent_ops(w: &mut World, actor: &str, prefix: &str, cmds: Vec<Command>) {
    for (i, c) in cmds.into_iter().enumerate() {
        let op = Op::new(format!("{prefix}:{i}"), c);
        w.server
            .submit_tool("paint", FILE, &Principal::new(WS, actor), vec![op])
            .unwrap();
    }
}

/// A command that the real server takes from an agent (a delete alone is held for approval).
fn nudge() -> Command {
    let patch = meldframe_protocol::NodePatch {
        x: Some(5.0),
        ..Default::default()
    };
    Command::UpdateNode {
        id: "t1".into(),
        patch,
    }
}

/// An event frame as the server would number it next: scripted, for commands the gate would hold back.
fn scripted(w: &mut World, i: usize, actor: &str, key: &str, command: Command) {
    let seq = w.app(i).state().confirmed_seq + 1;
    let key = Some(key.into());
    let frame = Frame::Event {
        seq,
        actor: actor.into(),
        key,
        command,
    };
    w.act(
        i,
        Action::Frame {
            now_ms: w.now,
            frame,
        },
    );
}

/// The fetches among the effects the client has returned since `from`.
fn fetches(w: &World, i: usize, from: usize) -> Vec<(Option<u32>, Option<u32>)> {
    let fx = &w.clients[i].effects[from..];
    fx.iter()
        .filter_map(|e| match e {
            Effect::FetchBatches {
                before_n, after_n, ..
            } => Some((*before_n, *after_n)),
            _ => None,
        })
        .collect()
}

fn count(w: &World, i: usize) -> Option<u32> {
    w.app(i).state().activity.unreviewed_count
}

#[test]
fn a_batch_with_nothing_to_show_is_still_revertable_and_a_header_only_row_is_not() {
    let mut w = World::new(64);
    w.now = T;
    let me = w.join("user:alice", "a", 1);
    let collection: Command = serde_json::from_value(json!({"type": "addCollection",
        "collection": {"id": "c1", "name": "brand", "modes": [{"id": "m1", "name": "Light"}]}}))
    .unwrap();
    let n1 = Command::AddNode {
        node: node("n1", "frame", Some("root")),
        index: None,
    };
    scripted(
        &mut w,
        me,
        "agent:x",
        "del:0",
        Command::RemoveNode { id: "t1".into() },
    );
    scripted(&mut w, me, "agent:x", "vars:0", collection);
    scripted(&mut w, me, "agent:x", "addrm:0", n1);
    scripted(
        &mut w,
        me,
        "agent:x",
        "addrm:1",
        Command::RemoveNode { id: "n1".into() },
    );
    w.act(me, frame(header(9, 1, "agent:z", "zz"), 1, T));
    let rows = &w.app(me).state().activity.rows;
    for p in ["del", "vars", "addrm"] {
        let r = rows.iter().find(|r| r.key_prefix == p).expect("a row");
        assert!(r.revertable, "{p}: the app holds every op of it");
    }
    let header_only = w
        .app(me)
        .state()
        .activity
        .row(&by("agent:z", "zz"))
        .unwrap();
    assert!(
        !header_only.revertable,
        "a header alone has no ops to revert"
    );
}

#[test]
fn the_answer_of_the_review_route_cannot_overwrite_the_count_of_a_newer_frame() {
    let mut w = World::new(64);
    w.now = T;
    let me = w.join("user:alice", "a", 1);
    w.act(me, frame(header(1, 1, "agent:x", "b1"), 1, T));
    let mut accepted = header(1, 2, "agent:x", "b1");
    accepted.review = ReviewState::Accepted;
    // The frame of the change comes first, with the right count; the HTTP answer after it, with the count the front
    // end had before.
    w.act(me, frame(accepted.clone(), 0, T));
    w.act(me, Action::BatchReviewed { batch: accepted });
    assert_eq!(count(&w, me), Some(0), "the count of the frame stays");
    assert_eq!(
        w.app(me)
            .state()
            .activity
            .row(&by("agent:x", "b1"))
            .unwrap()
            .review,
        ReviewState::Accepted
    );
}

#[test]
fn a_clock_that_runs_while_the_connection_is_down_spends_no_tries() {
    let mut w = World::new(64);
    w.now = T;
    let me = w.join("user:alice", "a", 1);
    agent_ops(&mut w, "agent:x", "b1", vec![nudge()]);
    w.pump_all();
    w.drop_link(me);
    for at in [3_000, 8_000, 13_000, 18_000] {
        w.act(me, Action::Tick { now_ms: T + at });
    }
    let row = |w: &World| w.app(me).state().activity.rows[0].status;
    assert_eq!(
        row(&w),
        BatchStatus::Building,
        "nothing was asked, so nothing was spent"
    );
    w.now = T + 19_000;
    let from = w.clients[me].effects.len();
    w.act(me, Action::Reconnect { now_ms: T + 19_000 });
    assert_eq!(
        fetches(&w, me, from).len(),
        1,
        "the subscribe reads the newest page, once"
    );
    // That fetch took the window: the first live tick does not ask again for the same headers, and spends no try.
    w.act(me, Action::Tick { now_ms: T + 19_500 });
    assert_eq!(fetches(&w, me, from).len(), 1);
    assert_eq!(row(&w), BatchStatus::Building);
    w.act(me, Action::Tick { now_ms: T + 24_100 });
    assert_eq!(
        fetches(&w, me, from).len(),
        2,
        "rule (b) asks once the window is over"
    );
    assert_eq!(row(&w), BatchStatus::Building);
}

#[test]
fn rule_a_asks_at_most_once_in_five_seconds() {
    let mut w = World::new(64);
    w.now = T;
    let me = w.join("user:alice", "a", 1);
    w.act(me, frame(header(1, 1, "agent:x", "b1"), 1, T));
    let from = w.clients[me].effects.len();
    w.act(me, frame(header(3, 3, "agent:x", "b3"), 1, T + 6_000));
    w.act(me, frame(header(6, 6, "agent:x", "b6"), 1, T + 6_100));
    assert_eq!(
        fetches(&w, me, from).len(),
        1,
        "the second gap is inside the window"
    );
    w.act(me, frame(header(9, 9, "agent:x", "b9"), 1, T + 12_000));
    assert_eq!(fetches(&w, me, from).len(), 2);
}

#[test]
fn an_answer_that_is_not_the_one_rule_d_waits_for_does_not_end_the_chain() {
    let mut w = World::new(64);
    w.now = T;
    let me = w.join("user:alice", "a", 1);
    let rows = |ns: &[u32]| {
        ns.iter()
            .map(|n| header(*n, *n as u64, "a:1", &format!("p{n}")))
            .collect::<Vec<_>>()
    };
    w.act(me, page(rows(&[3, 7]), false, 1));
    let from = w.clients[me].effects.len();
    w.act(
        me,
        Action::Frame {
            now_ms: T,
            frame: Frame::Resync {
                after_seq: 0,
                latest_seq: 0,
            },
        },
    );
    assert_eq!(fetches(&w, me, from), vec![(None, None)]);
    w.act(me, page(rows(&[9, 8, 7]), true, 4));
    assert_eq!(fetches(&w, me, from).last(), Some(&(Some(7), None)));
    // The answer of the review route, and a newest page asked for by another rule, arrive in the middle.
    w.act(
        me,
        Action::BatchReviewed {
            batch: header(7, 20, "a:1", "p7"),
        },
    );
    w.act(me, page(rows(&[12, 11, 10]), true, 4));
    let n = fetches(&w, me, from).len();
    w.act(me, page(rows(&[6, 5]), true, 4));
    assert_eq!(
        fetches(&w, me, from)[n..],
        [(Some(5), None)],
        "the chain goes on below 5"
    );
}

#[test]
fn a_reload_does_not_keep_the_old_in_effect_answer_whether_by_the_key_or_by_the_reset() {
    let mut w = World::new(64);
    w.now = T;
    let me = w.join("user:alice", "a", 1);
    agent_ops(&mut w, "agent:x", "b1", vec![nudge()]);
    w.pump_all();
    let h = header(1, 1, "agent:x", "b1");
    w.act(me, frame(h.clone(), 1, T));
    w.act(
        me,
        Action::SelectBatch {
            batch: by("agent:x", "b1"),
        },
    );
    assert_eq!(w.app(me).state().activity.rows[0].in_effect, Some(true));
    let (nodes, seq) = w.room();
    let reload = Action::Reload {
        now_ms: T,
        nodes,
        variables: Default::default(),
        seq,
        applied: vec![],
    };
    w.act(me, reload);
    assert!(w.app(me).state().activity.rows.is_empty());
    w.act(me, page(vec![h], false, 1));
    w.act(
        me,
        Action::SelectBatch {
            batch: by("agent:x", "b1"),
        },
    );
    let row = &w.app(me).state().activity.rows[0];
    assert_eq!(
        (row.revertable, row.in_effect),
        (false, None),
        "the log no longer holds the batch"
    );
}

fn full_page(hi: u32) -> Vec<BatchHeader> {
    (0..100)
        .map(|i| header(hi - i, u64::from(hi - i), "a:1", &format!("p{}", hi - i)))
        .collect()
}

#[test]
fn on_a_reconnect_the_small_answer_of_rule_b_first_does_not_end_the_newest_page_chain() {
    let mut w = World::new(64);
    w.now = T;
    let me = w.join("user:alice", "a", 1);
    w.act(me, page(vec![header(5, 5, "a:1", "p5")], false, 1));
    w.drop_link(me);
    w.now = T + 1_000;
    let from = w.clients[me].effects.len();
    w.act(me, Action::Reconnect { now_ms: T + 1_000 });
    assert_eq!(fetches(&w, me, from), vec![(None, None)]);
    // A fetch of rule (b) answered first (n above the highest held), then the newest page, which is full.
    w.act(me, page(vec![header(7, 7, "a:1", "p7")], false, 1));
    w.act(me, page(full_page(200), true, 2));
    assert_eq!(
        fetches(&w, me, from).last(),
        Some(&(Some(101), None)),
        "it still reads down"
    );
}

#[test]
fn batch_and_touch_frames_refresh_only_the_panel() {
    let mut w = World::new(64);
    w.now = T;
    let me = w.join("user:alice", "a", 1);
    w.act(
        me,
        Action::Select {
            ids: vec!["t1".into()],
        },
    );
    w.act(
        me,
        Action::Move {
            id: "t1".into(),
            x: 9.0,
            y: 0.0,
        },
    );
    assert!(
        !w.app(me).state().pending.is_empty(),
        "an op of the person is waiting for the server"
    );
    let before = w.clients[me].effects.len();
    let events = w.clients[me].events.len();
    let touch = Frame::Touch {
        actor: "agent:x".into(),
        ids: vec!["r1".into()],
        seq: 1,
    };
    w.act(
        me,
        Action::Frame {
            now_ms: T,
            frame: touch,
        },
    );
    w.act(me, frame(header(1, 1, "agent:x", "b1"), 1, T));
    let fx = &w.clients[me].effects[before..];
    assert!(fx.is_empty(), "no Render, no SendOps: {fx:?}");
    let ev = &w.clients[me].events[events..];
    assert_eq!(ev.len(), 2, "{ev:?}");
    assert!(
        ev.iter()
            .all(|e| *e == meldframe_app::Event::ActivityChanged {})
    );
    let a = &w.app(me).state().activity;
    assert_eq!(
        (a.touches.len(), a.rows.len(), a.unreviewed_count),
        (1, 1, Some(1)),
        "the panel was rebuilt"
    );
}
