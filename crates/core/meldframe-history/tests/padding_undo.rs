//! Undo and redo of padding per side: a side that was not set goes back to "not set", so it follows `padding` again.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{base, snap, upd};

#[test]
fn undo_of_setting_a_side_unsets_it_and_redo_sets_it_again() {
    let mut s = base();
    let before = snap(&s);
    s.apply(&upd("a", |p| p.pad_l = Some(Some(4.0)))).unwrap();
    assert_eq!(s.node("a").unwrap().pad_l, Some(4.0));
    assert!(s.undo().is_some());
    assert_eq!(s.node("a").unwrap().pad_l, None);
    assert_eq!(snap(&s), before);
    assert!(s.redo().is_some());
    assert_eq!(s.node("a").unwrap().pad_l, Some(4.0));
}

#[test]
fn undo_of_unsetting_a_side_restores_its_number() {
    let mut s = base();
    s.apply(&upd("a", |p| p.pad_t = Some(Some(2.0)))).unwrap();
    s.apply(&upd("a", |p| p.pad_t = Some(None))).unwrap();
    assert_eq!(s.node("a").unwrap().pad_t, None);
    s.undo().unwrap();
    assert_eq!(s.node("a").unwrap().pad_t, Some(2.0));
}

#[test]
fn two_changes_of_a_side_join_into_one_step_that_undoes_to_the_start() {
    let mut s = base();
    s.apply(&upd("a", |p| p.pad_r = Some(Some(1.0)))).unwrap();
    s.apply(&upd("a", |p| p.pad_r = Some(Some(7.0)))).unwrap();
    assert_eq!(s.node("a").unwrap().pad_r, Some(7.0));
    s.undo().unwrap();
    s.undo().unwrap();
    assert_eq!(s.node("a").unwrap().pad_r, None);
}
