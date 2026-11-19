//! Mode ids are per collection and are reused across collections. A remote `removeMode` takes away my
//! `setVariableValue` only for a variable of the collection that lost the mode.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_history::FileState;
use meldframe_protocol::Command;
use serde_json::json;

fn cmd(v: serde_json::Value) -> Command {
    serde_json::from_value(v).unwrap()
}

/// The collections `a` and `b`, each with the modes `d` (default) and `m0`, and a colour variable in each: `va`, `vb`.
fn file() -> FileState {
    let mut s = FileState::new();
    for (col, var) in [("a", "va"), ("b", "vb")] {
        s.apply(&cmd(
            json!({"type": "addCollection", "collection": {"id": col, "name": col,
            "modes": [{"id": "d", "name": "d"}, {"id": "m0", "name": "m0"}]}}),
        ))
        .unwrap();
        s.apply(&cmd(
            json!({"type": "addVariable", "variable": {"id": var, "collection": col,
            "name": "color/x", "type": "color", "values": {"d": "#111111", "m0": "#000000"}}}),
        ))
        .unwrap();
    }
    s
}

fn set(var: &str) -> Command {
    cmd(json!({"type": "setVariableValue", "id": var, "mode": "m0", "value": "#ff0000"}))
}

fn remove_m0(col: &str) -> Command {
    cmd(json!({"type": "removeMode", "collection": col, "mode": "m0"}))
}

#[test]
fn a_remote_remove_of_a_mode_in_another_collection_keeps_my_value_step() {
    let mut s = file();
    s.apply(&set("vb")).unwrap();
    let depth = s.undo_depth();
    s.apply_remote(&remove_m0("a")).unwrap();
    assert_eq!(s.undo_depth(), depth, "the step is for a variable of b");
    assert!(s.undo().is_some());
    let old = s.vars().variable("vb").unwrap().values["m0"].clone();
    assert_eq!(
        old,
        meldframe_protocol::Bind::Lit(meldframe_protocol::Value::Color("#000000".into()))
    );
}

#[test]
fn a_remote_remove_of_a_mode_in_the_collection_of_the_variable_drops_my_value_step() {
    let mut s = file();
    s.apply(&set("va")).unwrap();
    let depth = s.undo_depth();
    s.apply_remote(&remove_m0("a")).unwrap();
    assert_eq!(s.undo_depth(), depth - 1);
}
