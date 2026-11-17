//! Undo and redo of a margin patch: a plain number field, restored like any other.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{base, snap, upd};

#[test]
fn undo_puts_a_margin_back_and_redo_sets_it_again() {
    let mut s = base();
    let before = snap(&s);
    s.apply(&upd("a", |p| p.mar_t = Some(12.0))).unwrap();
    assert_eq!(s.node("a").unwrap().mar_t, 12.0);
    assert!(s.undo().is_some());
    assert_eq!(snap(&s), before);
    assert!(s.redo().is_some());
    assert_eq!(s.node("a").unwrap().mar_t, 12.0);
}
