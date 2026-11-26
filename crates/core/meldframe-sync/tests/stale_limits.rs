//! Limits of the correction of a pending undo (issue 148): a refused correction after the stale undo was
//! confirmed, a refusal that makes the replica forget what confirmed ops left in `base`, and two actors
//! with an undo on its way for the same field.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "common/mod.rs"]
mod common;

use common::{base, upd, x_of};
use meldframe_sync::Replica;

fn replica() -> Replica {
    Replica::new("user:a", "ca", base(), 0).unwrap()
}

#[test]
fn a_refused_last_correction_after_the_stale_undo_is_confirmed_leaves_the_other_actors_value() {
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    // The other actor's x=5 is ordered first; the stale undo (x=0) is already on its way.
    a.on_event(1, "user:b", &upd("a", |p| p.x = Some(5.0)))
        .unwrap();
    let fix = a.take_outbox();
    assert_eq!(fix.len(), 1, "one correction");
    a.on_event(2, "user:a", &sent[0].command).unwrap();
    a.on_event(3, "user:a", &sent[1].command).unwrap();
    assert_eq!(x_of(a.base(), "a"), 0.0, "the stale undo landed");
    assert!(a.on_reject(&fix[0].key));
    // The server refused the correction once; the replica sends it one more time, and a second refusal ends it.
    let again = a.take_outbox();
    assert_eq!(again.len(), 1, "the correction is sent once more");
    a.on_event(4, "user:a", &again[0].command).unwrap();
    assert_eq!(x_of(a.base(), "a"), 5.0);
    assert_eq!(a.pending_len(), 0);
    a.check_consistency().unwrap();
}

#[test]
fn a_correction_refused_twice_is_not_sent_a_third_time() {
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    a.on_event(1, "user:b", &upd("a", |p| p.x = Some(5.0)))
        .unwrap();
    let fix = a.take_outbox();
    a.on_event(2, "user:a", &sent[0].command).unwrap();
    a.on_event(3, "user:a", &sent[1].command).unwrap();
    assert!(a.on_reject(&fix[0].key));
    let again = a.take_outbox();
    assert_eq!(again.len(), 1);
    assert!(a.on_reject(&again[0].key));
    assert!(a.take_outbox().is_empty(), "the second refusal ends it");
    a.check_consistency().unwrap();
}

#[test]
fn a_refused_correction_whose_dependent_op_falls_is_still_sent_once_more() {
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    a.on_event(1, "user:b", &upd("a", |p| p.x = Some(5.0)))
        .unwrap();
    let fix = a.take_outbox();
    // A later op of mine names the correction as its dependency, so it falls with it.
    a.apply_dependent(&upd("k", |p| p.x = Some(3.0)), &[fix[0].key.clone()])
        .unwrap();
    let later = a.take_outbox();
    a.on_event(2, "user:a", &sent[0].command).unwrap();
    a.on_event(3, "user:a", &sent[1].command).unwrap();
    assert!(a.on_reject(&fix[0].key));
    assert_eq!(
        a.unconfirmed()
            .iter()
            .filter(|o| o.key == later[0].key)
            .count(),
        0,
        "the dependent op fell"
    );
    let again = a.take_outbox();
    assert_eq!(again.len(), 1, "the correction is sent once more");
    a.on_event(4, "user:a", &again[0].command).unwrap();
    assert_eq!(x_of(a.base(), "a"), 5.0);
    a.check_consistency().unwrap();
}

#[test]
fn a_refused_resend_takes_its_dependent_with_it_and_nothing_is_sent_again() {
    // Issue 164: a resend that the server refuses ends there, also when a later op of mine depended on it.
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    a.on_event(1, "user:b", &upd("a", |p| p.x = Some(5.0)))
        .unwrap();
    let fix = a.take_outbox();
    a.on_event(2, "user:a", &sent[0].command).unwrap();
    a.on_event(3, "user:a", &sent[1].command).unwrap();
    assert!(a.on_reject(&fix[0].key));
    let again = a.take_outbox();
    assert_eq!(again.len(), 1, "the correction is sent once more");
    // A later op of mine names the resend as its dependency.
    a.apply_dependent(&upd("k", |p| p.x = Some(3.0)), &[again[0].key.clone()])
        .unwrap();
    let later = a.take_outbox();
    assert_eq!(later.len(), 1);
    assert!(a.on_reject(&again[0].key));
    assert!(
        a.unconfirmed().iter().all(|o| o.key != later[0].key),
        "the dependent op fell with the resend"
    );
    assert!(
        a.take_outbox().is_empty(),
        "neither the resend nor its dependent is sent again"
    );
    a.check_consistency().unwrap();
}

/// A server in its smallest form, and a way to run two replicas against it.
struct Room {
    nodes: Vec<meldframe_protocol::Node>,
    log: Vec<(String, meldframe_protocol::Command)>,
}

impl Room {
    fn put(&mut self, who: &str, c: &meldframe_protocol::Command) {
        let lim = meldframe_core::Limits::default();
        meldframe_core::apply_command(&mut self.nodes, c, &lim).unwrap();
        self.log.push((who.to_string(), c.clone()));
    }

    fn feed(&self, r: &mut Replica) {
        for (i, (w, c)) in self.log.iter().enumerate().skip(r.seq() as usize) {
            r.on_event(i as u64 + 1, w, c).unwrap();
        }
    }
}

#[test]
fn two_actors_with_an_undo_on_its_way_for_the_same_field_keep_a_value_of_an_undone_edit() {
    let mut nodes = base();
    meldframe_core::normalize_positions(&mut nodes);
    let mut room = Room { nodes, log: vec![] };
    let mut a = Replica::new("user:a", "ca", base(), 0).unwrap();
    let mut b = Replica::new("user:b", "cb", base(), 0).unwrap();
    // a edits and the server orders it; b sees it, edits and undoes before it sees anything else.
    a.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    room.put("user:a", &a.take_outbox()[0].command);
    room.feed(&mut b);
    b.apply(&upd("a", |p| p.x = Some(2.0))).unwrap();
    assert!(b.undo().unwrap());
    let ob = b.take_outbox();
    // a undoes before it sees b's edit; the server orders b's edit before that undo.
    assert!(a.undo().unwrap());
    let ua = a.take_outbox();
    room.put("user:b", &ob[0].command);
    room.feed(&mut a);
    room.put("user:a", &ua[0].command);
    room.feed(&mut a);
    room.put("user:b", &ob[1].command);
    for _ in 0..4 {
        for (r, who) in [(&mut a, "user:a"), (&mut b, "user:b")] {
            room.feed(r);
            for o in r.take_outbox() {
                room.put(who, &o.command);
            }
        }
    }
    room.feed(&mut a);
    room.feed(&mut b);
    // Both edits were undone, but the field holds a's undone value: b's undo went back to what a wrote
    // before it, and a's undo, which b's edit came before, was set right to b's value, which b undid.
    // The replicas agree on it; neither can tell it is a value of an undone edit, so it is not corrected.
    assert_eq!(x_of(&room.nodes, "a"), 1.0);
    assert_eq!((x_of(a.nodes(), "a"), x_of(b.nodes(), "a")), (1.0, 1.0));
    assert_eq!((a.pending_len(), b.pending_len()), (0, 0));
}

/// `a` edits x and y (10), undoes it (the undo is on its way), and another actor wrote x=5 and y=6 before
/// both. Returns the ops sent (edit, undo) and the correction for x and y.
fn stale_xy() -> (
    Replica,
    Vec<meldframe_sync::Outgoing>,
    meldframe_sync::Outgoing,
) {
    let mut a = replica();
    a.apply(&upd("a", |p| {
        (p.x, p.y) = (Some(10.0), Some(10.0));
    }))
    .unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    let b = upd("a", |p| (p.x, p.y) = (Some(5.0), Some(6.0)));
    a.on_event(1, "user:b", &b).unwrap();
    let mut fix = a.take_outbox();
    assert_eq!(fix.len(), 1);
    (a, sent, fix.remove(0))
}

fn fields_of(c: &meldframe_protocol::Command) -> Vec<(String, f64)> {
    let meldframe_protocol::Command::UpdateNode { patch, .. } = c else {
        panic!("not an update");
    };
    let v = serde_json::to_value(patch).unwrap();
    let mut f: Vec<_> = v
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, n)| (k.clone(), n.as_f64().unwrap()))
        .collect();
    f.sort_by(|p, q| p.0.cmp(&q.0));
    f
}

#[test]
fn a_refused_correction_whose_fields_were_all_overwritten_remotely_owes_nothing_itself() {
    let (mut a, sent, fix) = stale_xy();
    // The follow-up writes x and y again, ordered before my ops: nothing is left of the first correction,
    // and the follow-up depends on it, so it falls with it.
    let b2 = upd("a", |p| (p.x, p.y) = (Some(7.0), Some(8.0)));
    a.on_event(2, "user:b", &b2).unwrap();
    assert_eq!(a.take_outbox().len(), 1, "the follow-up");
    a.on_event(3, "user:a", &sent[0].command).unwrap();
    a.on_event(4, "user:a", &sent[1].command).unwrap();
    assert!(a.on_reject(&fix.key));
    let again = a.take_outbox();
    // Only what the follow-up owed is sent, never the values of the refused correction (5 and 6).
    assert_eq!(again.len(), 1);
    assert_eq!(
        fields_of(&again[0].command),
        [("x".into(), 7.0), ("y".into(), 8.0)]
    );
    a.check_consistency().unwrap();
}

#[test]
fn a_resend_carries_exactly_the_fields_no_remote_op_wrote_since() {
    let (mut a, sent, fix) = stale_xy();
    // Another actor writes y again, ordered before my ops; the correction owes x only.
    a.on_event(2, "user:b", &upd("a", |p| p.y = Some(9.0)))
        .unwrap();
    let follow = a.take_outbox();
    assert_eq!(fields_of(&follow[0].command), [("y".into(), 9.0)]);
    a.on_event(3, "user:a", &sent[0].command).unwrap();
    a.on_event(4, "user:a", &sent[1].command).unwrap();
    assert!(a.on_reject(&fix.key));
    let mut again: Vec<_> = a
        .take_outbox()
        .iter()
        .flat_map(|o| fields_of(&o.command))
        .collect();
    again.sort_by(|p, q| p.0.cmp(&q.0));
    assert_eq!(again, [("x".into(), 5.0), ("y".into(), 9.0)]);
    a.check_consistency().unwrap();
}

#[test]
fn a_correction_that_falls_with_a_refused_stale_op_is_not_sent() {
    let (mut a, sent, fix) = stale_xy();
    assert!(a.on_reject(&sent[1].key), "the stale undo is refused");
    assert!(
        !a.unconfirmed().iter().any(|o| o.key == fix.key),
        "the correction fell with it"
    );
    assert!(a.take_outbox().is_empty(), "the stale value never landed");
    assert_eq!(a.pending_len(), 1, "only the edit is left");
    a.check_consistency().unwrap();
}

#[test]
fn a_resend_does_not_overwrite_a_pending_edit_of_the_same_value() {
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    a.on_event(1, "user:b", &upd("a", |p| p.x = Some(5.0)))
        .unwrap();
    let fix = a.take_outbox();
    a.on_event(2, "user:a", &sent[0].command).unwrap();
    a.on_event(3, "user:a", &sent[1].command).unwrap();
    // I now set x to 0 myself (the value the stale undo left), and an op that depends on the correction.
    a.apply(&upd("a", |p| p.x = Some(0.0))).unwrap();
    let mine = a.take_outbox();
    a.apply_dependent(&upd("k", |p| p.x = Some(3.0)), &[fix[0].key.clone()])
        .unwrap();
    a.take_outbox();
    assert!(a.on_reject(&fix[0].key));
    assert!(
        a.take_outbox().is_empty(),
        "my edit of x is not overwritten"
    );
    assert_eq!(a.unconfirmed().len(), mine.len());
    assert_eq!(x_of(a.nodes(), "a"), 0.0);
    a.check_consistency().unwrap();
}
