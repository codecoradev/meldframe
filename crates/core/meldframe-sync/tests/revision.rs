//! `Replica::revision` names the state of the view: it changes whenever `nodes()` or `vars()` can have, and only then,
//! so something worked out from the view (a layout) may be kept while it stays the same.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{base, remove, upd};
use meldframe_sync::Replica;

#[test]
fn it_changes_with_an_edit_an_undo_a_redo_and_an_event_of_another_actor() {
    let mut r = Replica::new("user:a", "ca", base(), 0).unwrap();
    let mut seen = vec![r.revision()];
    let mut next = |r: &Replica, what: &str| {
        let now = r.revision();
        assert!(!seen.contains(&now), "{what}: the revision did not change");
        seen.push(now);
    };
    r.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    next(&r, "an edit");
    r.undo().unwrap();
    next(&r, "an undo");
    r.redo().unwrap();
    next(&r, "a redo");
    r.on_event(1, "user:b", &upd("a", |p| p.y = Some(2.0)))
        .unwrap();
    next(&r, "an event of another actor");
}

#[test]
fn it_stays_while_nothing_changes_and_after_a_refused_command() {
    let mut r = Replica::new("user:a", "ca", base(), 0).unwrap();
    let at = r.revision();
    assert!(r.apply(&remove("no-such-node")).is_err());
    assert_eq!(r.revision(), at, "a refused command changes nothing");
    let _ = (r.nodes().len(), r.vars());
    assert_eq!(r.revision(), at, "reading changes nothing");
}

#[test]
fn a_reload_gives_a_revision_no_earlier_state_had_even_when_the_view_counts_again_from_the_start() {
    let mut r = Replica::new("user:a", "ca", base(), 0).unwrap();
    let first = r.revision();
    // The same file again, as a reload after a lost connection brings it: a new view whose own version starts over.
    r.resync(base(), 0, &[]).unwrap();
    assert_ne!(r.revision(), first);
}
