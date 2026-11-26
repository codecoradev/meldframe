//! A pending undo in a large shared file: remote events far from what the pending steps write leave the
//! corrections as they were, and a remote event on the same node still sets right the stale value.
//! (`field_diff` looks only at the nodes the ops touched; its debug check compares the result with a walk
//! of the whole tree, so every event here also proves the two agree.)

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "common/mod.rs"]
mod common;

use common::{node, upd, x_of};
use meldframe_core::{Limits, apply_command, normalize_positions};
use meldframe_protocol::{Command, Node};
use meldframe_sync::Replica;

const N: usize = 5_000;

fn big() -> Vec<Node> {
    let mut v = vec![node("root", "frame", None)];
    v.extend((0..N).map(|i| node(&format!("n{i}"), "rect", Some("root"))));
    normalize_positions(&mut v);
    v
}

fn set_x(id: &str, x: f64) -> Command {
    upd(id, |p| p.x = Some(x))
}

#[test]
fn remote_events_elsewhere_cost_no_correction_and_leave_the_pending_undo_alone() {
    let mut server = big();
    let mut a = Replica::new("user:a", "ca", server.clone(), 0).unwrap();
    a.apply(&set_x("n10", 5.0)).unwrap();
    let edit = a.take_outbox();
    assert!(a.undo().unwrap());
    let undo = a.take_outbox();
    let sent = a.pending_len();
    let lim = Limits::default();
    // My edit and my undo reach the server first; the events of others on other nodes come after.
    let mut log: Vec<(&str, Command)> = vec![];
    for o in edit.iter().chain(undo.iter()) {
        log.push(("user:a", o.command.clone()));
    }
    for i in 100..140 {
        log.push(("user:b", set_x(&format!("n{i}"), i as f64)));
    }
    for (i, (who, c)) in log.iter().enumerate() {
        apply_command(&mut server, c, &lim).unwrap();
        a.on_event(i as u64 + 1, who, c).unwrap();
    }
    assert_eq!(a.pending_len(), 0, "nothing was owed, nothing is left");
    assert!(a.take_outbox().is_empty());
    assert_eq!(x_of(a.nodes(), "n10"), x_of(&server, "n10"));
    assert_eq!(x_of(a.nodes(), "n120"), 120.0);
    assert!(sent >= 2);
}

#[test]
fn a_remote_write_to_the_node_of_a_pending_undo_still_sets_it_right() {
    let mut server = big();
    let mut a = Replica::new("user:a", "ca", server.clone(), 0).unwrap();
    let lim = Limits::default();
    a.apply(&set_x("n3000", 5.0)).unwrap();
    let edit = a.take_outbox();
    apply_command(&mut server, &edit[0].command, &lim).unwrap();
    a.on_event(1, "user:a", &edit[0].command).unwrap();
    assert!(a.undo().unwrap());
    let undo = a.take_outbox();
    // Another actor writes the same field before my undo is ordered: the undo I sent is stale.
    let theirs = set_x("n3000", 9.0);
    apply_command(&mut server, &theirs, &lim).unwrap();
    a.on_event(2, "user:b", &theirs).unwrap();
    for o in &undo {
        apply_command(&mut server, &o.command, &lim).unwrap();
        a.on_event(3, "user:a", &o.command).unwrap();
    }
    for o in a.take_outbox() {
        apply_command(&mut server, &o.command, &lim).unwrap();
    }
    assert_eq!(x_of(a.nodes(), "n3000"), x_of(&server, "n3000"));
}
