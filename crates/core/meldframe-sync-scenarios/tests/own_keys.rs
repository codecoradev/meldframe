//! A replica recognises its own ops by idempotency key (issue 90): a second session of the same actor,
//! or a repeated identical command, is not taken for the ack of my pending op.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-sync/tests/common/mod.rs"]
mod common;

use common::{base, snap, upd, x_of};
use meldframe_sync::{EventOutcome, Replica, ReplicaError};

const ACTOR: &str = "user:a";

/// Two sessions of one actor, each with a key prefix of its own, that send the same command.
fn two_sessions() -> (Replica, Replica) {
    let mut s1 = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    let mut s2 = Replica::new(ACTOR, "s2", base(), 0).unwrap();
    let c = upd("a", |p| p.x = Some(5.0));
    s1.apply(&c).unwrap();
    s2.apply(&c).unwrap();
    (s1, s2)
}

#[test]
fn two_sessions_of_one_actor_sending_the_same_command_are_each_acked_in_order() {
    let (mut s1, mut s2) = two_sessions();
    let (o1, o2) = (s1.take_outbox().remove(0), s2.take_outbox().remove(0));
    // The server numbers session 2's op first, then session 1's: same actor, same command.
    let (rebased, confirmed) = (
        EventOutcome::Rebased { dropped: 0 },
        EventOutcome::Confirmed,
    );
    for (r, want, left) in [
        (&mut s1, [rebased.clone(), confirmed.clone()], [1, 0]),
        (&mut s2, [confirmed, rebased], [0, 0]),
    ] {
        for (i, o) in [&o2, &o1].into_iter().enumerate() {
            let seq = i as u64 + 1;
            let got = r.on_event_keyed(seq, ACTOR, Some(&o.key), &o.command);
            assert_eq!(got.unwrap(), want[i], "seq {seq}");
            assert_eq!(r.pending_len(), left[i], "after seq {seq}");
        }
        assert_eq!(r.seq(), 2);
        r.check_consistency().unwrap();
        assert_eq!(x_of(r.nodes(), "a"), 5.0);
    }
    assert_eq!(snap(s1.base()), snap(s2.base()));
}

#[test]
fn identical_commands_of_one_replica_are_confirmed_in_key_order() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    for _ in 0..3 {
        r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    }
    let ops = r.take_outbox();
    for (i, o) in ops.iter().enumerate() {
        let got = r
            .on_event_keyed(i as u64 + 1, ACTOR, Some(&o.key), &o.command)
            .unwrap();
        assert_eq!(got, EventOutcome::Confirmed);
        assert_eq!(r.pending_len(), 2 - i);
    }
}

#[test]
fn an_event_of_my_actor_with_a_key_that_is_not_pending_is_another_session() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    // Same actor and same command as my pending op, but another key: not my ack.
    let c = upd("a", |p| p.x = Some(5.0));
    let got = r.on_event_keyed(1, ACTOR, Some("s2-1"), &c).unwrap();
    assert_eq!(got, EventOutcome::Rebased { dropped: 0 });
    assert_eq!(
        r.pending_len(),
        1,
        "my op is still waiting for its own event"
    );
    let key = r.unconfirmed()[0].key.clone();
    let got = r.on_event_keyed(2, ACTOR, Some(&key), &c).unwrap();
    assert_eq!(got, EventOutcome::Confirmed);
    assert_eq!(r.pending_len(), 0);
}

#[test]
fn an_event_without_a_key_is_matched_by_actor_and_command_as_before() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let c = upd("a", |p| p.x = Some(5.0));
    assert_eq!(r.on_event(1, ACTOR, &c).unwrap(), EventOutcome::Confirmed);
    r.apply(&upd("a", |p| p.x = Some(6.0))).unwrap();
    let got = r.on_event_keyed(2, ACTOR, None, &upd("a", |p| p.x = Some(6.0)));
    assert_eq!(got.unwrap(), EventOutcome::Confirmed);
    assert_eq!(r.pending_len(), 0);
}

#[test]
fn an_op_of_mine_ordered_ahead_of_an_older_one_is_reported_so_the_caller_can_resync() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    r.apply(&upd("a", |p| p.y = Some(7.0))).unwrap();
    let ops = r.take_outbox();
    // The first was refused (or is late), the second got seq 1: it is mine, but not the oldest pending op.
    let got = r.on_event_keyed(1, ACTOR, Some(&ops[1].key), &ops[1].command);
    assert!(matches!(got, Err(ReplicaError::Diverged(_))), "{got:?}");
    assert_eq!((r.seq(), r.pending_len()), (0, 2), "nothing was changed");
    r.check_consistency().unwrap();
    let mut snap = base();
    snap.iter_mut().find(|n| n.id == "a").unwrap().y = 7.0;
    r.resync(snap, 1, &[ops[1].key.clone()]).unwrap();
    assert_eq!(
        r.unconfirmed()[0].key,
        ops[0].key,
        "the older op is sent again"
    );
    assert_eq!(r.pending_len(), 1);
}
