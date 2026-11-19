//! E2.4: an agent cannot fill the memory of the server with approvals, and ids cannot be guessed.

use common::*;
use meldframe_approvals::{ApprovalRefusal, Approvals, Verdict};
use meldframe_host::Principal;

mod common;

fn ask_as(
    approvals: &mut Approvals,
    room: &meldframe_host::Room<meldframe_host::MemoryStore>,
    actor: &str,
    key: &str,
) -> Result<meldframe_host::Report, ApprovalRefusal> {
    let mut gate = dflt(approvals);
    let who = Principal::new("ws_1", actor);
    submit_as(approvals, &mut gate, &who, room, rm(key, "f"), 10)
}

fn is_required(r: Result<meldframe_host::Report, ApprovalRefusal>) -> bool {
    matches!(r, Err(ApprovalRefusal::Required { .. }))
}

#[test]
fn an_actor_holds_a_limited_number_of_open_approvals() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    for i in 0..Approvals::MAX_OPEN_PER_ACTOR {
        assert!(is_required(ask_as(
            &mut approvals,
            &room,
            "agent:a",
            &format!("k{i}")
        )));
    }
    let over = ask_as(&mut approvals, &room, "agent:a", "one-more").unwrap_err();
    assert_eq!(
        over,
        ApprovalRefusal::TooMany {
            limit: Approvals::MAX_OPEN_PER_ACTOR as u64
        }
    );
    assert_eq!(over.code(), "approval_limit");
    // Asking again for one that is open is no new approval, so it still answers `approval_required`.
    assert!(is_required(ask_as(&mut approvals, &room, "agent:a", "k0")));
    // Another actor is not held back by it.
    assert!(is_required(ask_as(&mut approvals, &room, "agent:b", "k0")));
}

#[test]
fn an_answered_approval_frees_its_place() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    for i in 0..Approvals::MAX_OPEN_PER_ACTOR {
        ask_as(&mut approvals, &room, "agent:a", &format!("k{i}")).unwrap_err();
    }
    let first = approvals.pending("ws_1")[0].id.clone();
    approvals
        .decide(&first, Verdict::Deny, "owner:o", 20)
        .unwrap();
    assert!(is_required(ask_as(
        &mut approvals,
        &room,
        "agent:a",
        "one-more"
    )));
}

#[test]
fn a_workspace_holds_a_limited_number_of_open_approvals() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let per = Approvals::MAX_OPEN_PER_ACTOR;
    let mut n = 0;
    'fill: for actor in 0.. {
        for k in 0..per {
            if n == Approvals::MAX_OPEN_PER_WORKSPACE {
                break 'fill;
            }
            assert!(is_required(ask_as(
                &mut approvals,
                &room,
                &format!("agent:{actor}"),
                &format!("k{k}")
            )));
            n += 1;
        }
    }
    let over = ask_as(&mut approvals, &room, "agent:fresh", "k").unwrap_err();
    assert_eq!(
        over,
        ApprovalRefusal::TooMany {
            limit: Approvals::MAX_OPEN_PER_WORKSPACE as u64
        }
    );
    // Another workspace is not held back.
    let mut gate = dflt(&approvals);
    let other = Principal::new("ws_2", "agent:fresh");
    let r = submit_as(&mut approvals, &mut gate, &other, &room, rm("k", "f"), 10);
    assert!(is_required(r));
}

#[test]
fn ids_are_not_a_count_and_differ_between_runs() {
    let (room, mut one, mut two) = (room(), Approvals::new(TTL), Approvals::new(TTL));
    let a = ask(&mut one, &room, "k1", "f", 10);
    let b = ask(&mut one, &room, "k2", "f", 10);
    let c = ask(&mut two, &room, "k1", "f", 10);
    for id in [&a, &b, &c] {
        assert!(id.starts_with("apr_") && id.len() == 36, "{id}");
        assert!(id[4..].bytes().all(|b| b.is_ascii_hexdigit()), "{id}");
    }
    assert!(a != b && a != c, "{a} {b} {c}");
}

#[test]
fn the_whole_set_is_bounded_and_prune_makes_room() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    for i in 0..Approvals::MAX_ITEMS {
        let id = ask(&mut approvals, &room, &format!("k{i}"), "f", 10);
        approvals.decide(&id, Verdict::Deny, "owner:o", 11).unwrap();
    }
    let over = ask_as(&mut approvals, &room, "agent:a", "late").unwrap_err();
    assert_eq!(
        over,
        ApprovalRefusal::TooMany {
            limit: Approvals::MAX_ITEMS as u64
        }
    );
    approvals.prune(10 + 2 * TTL);
    assert!(is_required(ask_as(
        &mut approvals,
        &room,
        "agent:a",
        "late"
    )));
}

#[test]
fn a_key_that_is_too_long_opens_no_approval() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let long = "k".repeat(Approvals::MAX_KEY_CHARS + 1);
    let r = ask_as(&mut approvals, &room, "agent:a", &long).unwrap_err();
    assert_eq!(r.code(), "forbidden");
    assert!(approvals.pending("ws_1").is_empty());
    let edge = "k".repeat(Approvals::MAX_KEY_CHARS);
    assert!(is_required(ask_as(&mut approvals, &room, "agent:a", &edge)));
}

#[test]
fn the_limit_message_does_not_tell_how_many_are_open() {
    let m = ApprovalRefusal::TooMany { limit: 2000 }.to_string();
    assert!(!m.bytes().any(|b| b.is_ascii_digit()), "{m}");
}
