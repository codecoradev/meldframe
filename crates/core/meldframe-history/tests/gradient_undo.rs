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
