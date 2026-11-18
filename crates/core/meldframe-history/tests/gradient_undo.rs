//! Undo and redo of a gradient: setting, replacing and removing it are each one step that goes back exactly.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{base, snap, upd};
use meldframe_protocol::Gradient;

fn g(color: &str) -> Gradient {
    serde_json::from_value(
        serde_json::json!({"stops": [{"pos": 0, "color": color}, {"pos": 1, "color": "#000000"}]}),
    )
    .unwrap()
}

#[test]
fn undo_of_setting_a_gradient_removes_it_and_redo_sets_it() {
    let mut s = base();
    let before = snap(&s);
    s.apply(&upd("a", |p| p.gradient = Some(Some(g("#ffffff")))))
        .unwrap();
    assert!(s.node("a").unwrap().gradient.is_some());
    s.undo().unwrap();
    assert!(s.node("a").unwrap().gradient.is_none());
    assert_eq!(snap(&s), before);
    s.redo().unwrap();
    assert_eq!(s.node("a").unwrap().gradient, Some(g("#ffffff")));
}

#[test]
fn undo_of_removing_or_replacing_restores_the_old_gradient() {
    let mut s = base();
    s.apply(&upd("a", |p| p.gradient = Some(Some(g("#111111")))))
        .unwrap();
    s.apply(&upd("a", |p| p.gradient = Some(Some(g("#222222")))))
        .unwrap();
    s.apply(&upd("a", |p| p.gradient = Some(None))).unwrap();
    assert!(s.node("a").unwrap().gradient.is_none());
    s.undo().unwrap();
    assert_eq!(s.node("a").unwrap().gradient, Some(g("#222222")));
    s.undo().unwrap();
    assert_eq!(s.node("a").unwrap().gradient, Some(g("#111111")));
}

#[test]
fn a_remote_edit_of_another_field_keeps_the_undo_and_redo_of_my_gradient() {
    let mut s = base();
    s.apply(&upd("a", |p| p.gradient = Some(Some(g("#ffffff")))))
        .unwrap();
    // Another actor writes a different field of the same node: my step still holds the gradient.
    s.apply_remote(&upd("a", |p| p.x = Some(12.0))).unwrap();
    assert!(s.undo().is_some(), "the gradient step is still there");
    let a = s.node("a").unwrap();
    assert!(a.gradient.is_none(), "my gradient is undone");
    assert_eq!(a.x, 12.0, "the other actor's field stays");
    s.apply_remote(&upd("a", |p| p.y = Some(3.0))).unwrap();
    assert!(s.redo().is_some(), "the redo step is still there");
    assert_eq!(s.node("a").unwrap().gradient, Some(g("#ffffff")));
}
