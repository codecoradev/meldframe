//! E3.3: variables in a `Replica`. My variable commands apply at once and are undone like node commands; another
//! actor's variable command reaches the confirmed state and the view in the server's order, and my pending
//! ops are applied again on top of it.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{add, bg, bind_fill, cmd, set_bg, theme};
use meldframe_protocol::{Bind, Value};
use meldframe_sync::{EventOutcome, Replica};
use serde_json::json;

fn dark(r: &Replica) -> Bind<Value> {
    r.vars().variable("bg").unwrap().values["dark"].clone()
}

fn color(c: &str) -> Bind<Value> {
    Bind::Lit(Value::Color(c.into()))
}

fn replica() -> Replica {
    let mut r = Replica::new("user:a", "ca", vec![], 0).unwrap();
    r.apply(&theme()).unwrap();
    r.apply(&bg()).unwrap();
    r
}

#[test]
fn my_variable_commands_apply_at_once_queue_as_ops_and_undo() {
    let mut r = replica();
    assert_eq!(r.take_outbox().len(), 2);
    r.apply(&set_bg("dark", "#111111")).unwrap();
    r.apply(&add("n", "rect", None)).unwrap();
    r.apply(&bind_fill("n", "bg")).unwrap();
    assert_eq!(dark(&r), color("#111111"));
    assert_eq!(r.pending_len(), 5, "nothing is confirmed yet");
    // Undo walks back through the binding, the add of the node and the value.
    assert!(r.undo().unwrap() && r.undo().unwrap() && r.undo().unwrap());
    assert_eq!(dark(&r), color("#000000"));
    r.check_consistency().unwrap();
}

#[test]
fn a_remote_variable_command_moves_the_confirmed_state_and_my_pending_ops_stay_on_top() {
    let mut r = Replica::new("user:a", "ca", vec![], 0).unwrap();
    assert_eq!(
        r.on_event(1, "user:b", &theme()),
        Ok(EventOutcome::Rebased { dropped: 0 })
    );
    assert_eq!(
        r.on_event(2, "user:b", &bg()),
        Ok(EventOutcome::Rebased { dropped: 0 })
    );
    // I set the light value; the other actor sets the dark one before it.
    r.apply(&set_bg("light", "#eeeeee")).unwrap();
    r.on_event(3, "user:b", &set_bg("dark", "#222222")).unwrap();
    assert_eq!(dark(&r), color("#222222"));
    assert_eq!(
        r.vars().variable("bg").unwrap().values["light"],
        color("#eeeeee")
    );
    assert_eq!(r.pending_len(), 1, "my op still waits for the server");
    r.check_consistency().unwrap();
}

#[test]
fn a_pending_op_that_a_remote_variable_command_makes_invalid_is_dropped() {
    let mut r = Replica::new("user:a", "ca", vec![], 0).unwrap();
    r.on_event(1, "user:b", &theme()).unwrap();
    r.on_event(2, "user:b", &bg()).unwrap();
    r.apply(&set_bg("dark", "#111111")).unwrap();
    // The other actor removes the variable first: my value no longer applies.
    let out = r
        .on_event(
            3,
            "user:b",
            &cmd(json!({"type": "removeVariable", "id": "bg"})),
        )
        .unwrap();
    assert_eq!(out, EventOutcome::Rebased { dropped: 1 });
    assert!(r.vars().variable("bg").is_none());
    r.check_consistency().unwrap();
}

#[test]
fn a_snapshot_with_variables_loads_and_a_bad_one_is_refused() {
    let doc = {
        let mut r = replica();
        r.take_outbox();
        meldframe_protocol::Doc {
            vars: r.vars().clone(),
            ..Default::default()
        }
    };
    let r = Replica::from_doc("user:a", "ca", doc.clone(), 5).unwrap();
    assert_eq!(r.vars(), &doc.vars);
    let mut bad = doc;
    bad.vars.variables[0].values.remove("dark");
    assert_eq!(
        Replica::from_doc("user:a", "ca", bad, 5)
            .unwrap_err()
            .code(),
        "E_INCOMPLETE_VALUES"
    );
}
