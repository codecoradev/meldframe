//! An undo or redo that is still pending when another actor's op arrives. The server orders that op
//! before my pending ops, so the undo (or redo) I already sent was worked out from a premise that no
//! longer holds: it must not write a value that predates the other actor's, and the redo step must not
//! be lost. An op that is on its way cannot be recalled, so the replica sends the correction.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-sync/tests/common/mod.rs"]
mod common;

use common::{base, upd, x_of};
use meldframe_core::{Limits, apply_command, normalize_positions};
use meldframe_protocol::{Command, Node};
use meldframe_sync::{Outgoing, Replica};

/// The server in its smallest form: one total order, every op applied as it comes.
struct Server {
    nodes: Vec<Node>,
    log: Vec<(String, Command)>,
}

impl Server {
    fn new() -> Self {
        let mut nodes = base();
        normalize_positions(&mut nodes);
        Server { nodes, log: vec![] }
    }

    fn put(&mut self, actor: &str, c: &Command) {
        apply_command(&mut self.nodes, c, &Limits::default()).unwrap();
        self.log.push((actor.to_string(), c.clone()));
    }

    /// The server takes every op in the list, in order.
    fn take(&mut self, actor: &str, ops: &[Outgoing]) {
        ops.iter().for_each(|o| self.put(actor, &o.command));
    }

    /// Send the replica the events it has not seen.
    fn feed(&self, r: &mut Replica) {
        for (i, (a, c)) in self.log.iter().enumerate().skip(r.seq() as usize) {
            r.on_event(i as u64 + 1, a, c).unwrap();
        }
    }

    fn x(&self) -> f64 {
        x_of(&self.nodes, "a")
    }
}

fn replica() -> Replica {
    Replica::new("user:a", "ca", base(), 0).unwrap()
}

#[test]
fn a_pending_undo_restores_the_value_another_actor_wrote_before_my_edit() {
    let mut s = Server::new();
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    assert_eq!(sent.len(), 2, "the edit and its undo are on their way");
    // The server orders the other actor's x=5 first.
    s.put("user:b", &upd("a", |p| p.x = Some(5.0)));
    s.feed(&mut a);
    assert!(
        a.can_redo(),
        "my redo step keeps its field: the other write is ordered before my edit"
    );
    s.take("user:a", &sent);
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(s.x(), 5.0, "the undo went back to the other actor's value");
    assert_eq!((a.pending_len(), x_of(a.base(), "a")), (0, 5.0));
    assert_eq!(x_of(a.nodes(), "a"), 5.0);
    a.check_consistency().unwrap();
    // The redo still brings my value back, and undoing it goes back to the other actor's value again.
    assert!(a.redo().unwrap());
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(s.x(), 10.0);
    assert!(a.undo().unwrap());
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(s.x(), 5.0);
}

#[test]
fn an_undo_and_a_redo_that_are_both_on_their_way_keep_the_other_actors_value_for_the_next_undo() {
    let mut s = Server::new();
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    assert!(a.redo().unwrap());
    let sent = a.take_outbox();
    assert_eq!(sent.len(), 3);
    s.put("user:b", &upd("a", |p| p.x = Some(5.0)));
    s.feed(&mut a);
    assert!(
        a.can_undo(),
        "the redo's step must not lose its only field either"
    );
    s.take("user:a", &sent);
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(s.x(), 10.0, "my redo is the last write");
    assert!(a.undo().unwrap());
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(s.x(), 5.0, "undo goes back to what the other actor wrote");
}

#[test]
fn a_pending_undo_does_not_overwrite_a_write_ordered_after_my_confirmed_edit() {
    let mut s = Server::new();
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    let e = a.take_outbox();
    s.take("user:a", &e);
    s.feed(&mut a);
    assert!(a.undo().unwrap());
    let undo = a.take_outbox();
    // The server orders the other actor's x=5 between my edit and my undo.
    s.put("user:b", &upd("a", |p| p.x = Some(5.0)));
    s.feed(&mut a);
    s.take("user:a", &undo);
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(s.x(), 5.0, "undo never overwrites what the other actor did");
    assert_eq!((a.pending_len(), x_of(a.base(), "a")), (0, 5.0));
}

#[test]
fn a_remote_op_on_another_field_adds_no_op() {
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    a.take_outbox();
    let other = upd("a", |p| p.y = Some(7.0));
    a.on_event(1, "user:b", &other).unwrap();
    assert!(a.take_outbox().is_empty(), "nothing needs correcting");
    assert_eq!(a.pending_len(), 2);
    assert!(a.can_redo());
}

#[test]
fn a_correction_falls_with_the_edit_the_server_refuses() {
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    a.on_event(1, "user:b", &upd("a", |p| p.x = Some(5.0)))
        .unwrap();
    assert_eq!(a.take_outbox().len(), 1, "one correction");
    assert!(a.on_reject(&sent[0].key));
    assert_eq!(a.pending_len(), 0, "undo and correction fall with the edit");
    a.check_consistency().unwrap();
    assert_eq!(x_of(a.nodes(), "a"), 5.0);
}

#[test]
fn an_edit_made_after_a_stale_undo_undoes_to_the_other_actors_value() {
    let mut s = Server::new();
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    a.apply(&upd("a", |p| p.x = Some(30.0))).unwrap();
    let sent = a.take_outbox();
    assert_eq!(sent.len(), 3, "edit, undo, edit are on their way");
    s.put("user:b", &upd("a", |p| p.x = Some(5.0)));
    s.feed(&mut a);
    assert!(
        a.take_outbox().is_empty(),
        "the last edit writes 30 in both views: nothing to correct"
    );
    s.take("user:a", &sent);
    s.feed(&mut a);
    assert_eq!(s.x(), 30.0);
    // The undo of the second edit goes back to what the other actor wrote, not to the stale 0.
    assert!(a.undo().unwrap());
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(s.x(), 5.0);
    assert_eq!(x_of(a.nodes(), "a"), 5.0);
}

#[test]
fn a_refused_correction_is_not_sent_again() {
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert!(a.undo().unwrap());
    a.take_outbox();
    a.on_event(1, "user:b", &upd("a", |p| p.x = Some(5.0)))
        .unwrap();
    let mut open = a.take_outbox();
    assert_eq!(open.len(), 1, "one correction");
    // A server that refuses every correction: the replica must stop after the first refusal.
    let mut refusals = 0;
    while let Some(o) = open.pop() {
        refusals += 1;
        assert!(
            refusals < 5,
            "the refused correction is sent again and again"
        );
        assert!(a.on_reject(&o.key));
        open = a.take_outbox();
    }
    assert_eq!(refusals, 1);
    a.check_consistency().unwrap();
}

#[test]
fn a_stale_image_is_set_right_like_any_other_field() {
    // An asset id is 64 lowercase hex characters.
    let image =
        |c: &str| serde_json::from_value(serde_json::json!({"assetId": c.repeat(64)})).unwrap();
    let mut s = Server::new();
    let mut a = replica();
    a.apply(&upd("a", |p| p.image = Some(Some(image("a")))))
        .unwrap();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    s.put("user:b", &upd("a", |p| p.image = Some(Some(image("b")))));
    s.feed(&mut a);
    s.take("user:a", &sent);
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    let asset = |nodes: &[Node]| {
        let n = nodes.iter().find(|n| n.id == "a").unwrap();
        n.image.as_ref().map(|i| i.asset_id.clone())
    };
    assert_eq!(asset(&s.nodes).as_deref(), Some("b".repeat(64).as_str()));
    assert_eq!(asset(a.nodes()).as_deref(), Some("b".repeat(64).as_str()));
}

#[test]
fn a_stale_undo_the_server_applied_does_not_reach_the_undo_of_a_later_edit() {
    // The stale undo (x=0) is confirmed while my later edit is still pending; another actor's op then
    // arrives. The undo of the later edit must still go back to the other actor's x, not to the 0 that
    // the server holds.
    let mut s = Server::new();
    let mut a = replica();
    a.apply(&upd("a", |p| p.x = Some(100.0))).unwrap();
    assert!(a.undo().unwrap());
    a.apply(&upd("a", |p| p.x = Some(107.0))).unwrap();
    let sent = a.take_outbox();
    s.put("user:b", &upd("a", |p| p.x = Some(200.0)));
    s.feed(&mut a);
    s.put("user:a", &sent[0].command);
    s.put("user:a", &sent[1].command);
    s.feed(&mut a);
    assert_eq!(s.x(), 0.0, "the stale undo landed");
    s.put("user:b", &upd("a", |p| p.y = Some(9.0)));
    s.feed(&mut a);
    s.put("user:a", &sent[2].command);
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert!(a.undo().unwrap());
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(s.x(), 200.0, "undo goes back to what the other actor wrote");
}

#[test]
fn an_op_of_a_step_that_the_server_refuses_is_not_written_by_a_correction() {
    // A two-op undo: the first op is applied, the second is refused. The refused op's value must not be
    // written later by a correction built from what the step would have done.
    let mut s = Server::new();
    let mut a = replica();
    a.begin_group();
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    a.apply(&upd("k", |p| p.x = Some(20.0))).unwrap();
    a.end_group();
    assert!(a.undo().unwrap());
    let sent = a.take_outbox();
    assert_eq!(sent.len(), 4, "two edits and the two ops of their undo");
    s.put("user:b", &upd("a", |p| p.x = Some(5.0)));
    s.feed(&mut a);
    a.take_outbox();
    for o in &sent[..3] {
        s.put("user:a", &o.command);
    }
    s.feed(&mut a);
    // A later edit of mine is still pending when the refusal comes.
    a.apply(&upd("p", |p| p.x = Some(3.0))).unwrap();
    let later = a.take_outbox();
    // The server refuses the last op of the step (the undo of a.x); the edit of a.x stays.
    assert!(a.on_reject(&sent[3].key));
    s.put("user:b", &upd("p", |p| p.y = Some(1.0)));
    s.feed(&mut a);
    s.take("user:a", &later);
    s.take("user:a", &a.take_outbox());
    s.feed(&mut a);
    assert_eq!(
        s.x(),
        10.0,
        "the refused undo is not written by a correction"
    );
    assert_eq!(x_of(a.nodes(), "a"), 10.0);
}
