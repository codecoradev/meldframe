//! Groups (one step for many commands, such as a drag) and keyed edits (typing merges into one step).

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{add, base, snap, upd};
use meldframe_history::FileState;
use meldframe_protocol::Command;

fn set_x(s: &mut FileState, id: &str, x: f64) {
    s.apply(&upd(id, |p| p.x = Some(x))).unwrap();
}

fn set_text(s: &mut FileState, text: &str, key: Option<&str>) {
    s.apply_keyed(&upd("t", |p| p.text = Some(text.into())), key)
        .unwrap();
}

fn with_text_node() -> FileState {
    let mut s = base();
    s.apply(&add("t", "text", Some("root"))).unwrap();
    s
}

#[test]
fn a_group_is_one_step_that_undoes_and_redoes_as_a_whole() {
    let mut s = base();
    s.begin_group();
    for x in 1..=50 {
        set_x(&mut s, "a", f64::from(x));
    }
    s.end_group();
    assert_eq!(s.undo_depth(), 5, "four adds and one group");
    assert_eq!(s.node("a").unwrap().x, 50.0);
    s.undo().unwrap();
    assert_eq!(s.node("a").unwrap().x, 0.0);
    s.redo().unwrap();
    assert_eq!(s.node("a").unwrap().x, 50.0);
}

#[test]
fn undo_and_redo_of_a_group_apply_a_compacted_command_list() {
    let mut s = base();
    s.begin_group();
    for x in 1..=50 {
        set_x(&mut s, "a", f64::from(x));
    }
    s.end_group();
    let undone = s.undo().unwrap();
    assert!(matches!(undone.as_slice(), [Command::UpdateNode { id, .. }] if id == "a"));
    let redone = s.redo().unwrap();
    assert_eq!(redone.len(), 1, "fifty updates of one node become one");
}

#[test]
fn a_group_restores_a_mixed_run_in_reverse_order() {
    let mut s = base();
    let before = snap(&s);
    s.begin_group();
    s.apply(&add("n", "rect", Some("a"))).unwrap();
    s.apply(&upd("n", |p| p.fill = Some("#ff0000".into())))
        .unwrap();
    s.apply(&Command::Reparent {
        id: "n".into(),
        parent_id: Some("p".into()),
        pos: None,
    })
    .unwrap();
    s.apply(&Command::RemoveNode { id: "k".into() }).unwrap();
    s.end_group();
    let after = snap(&s);
    s.undo().unwrap();
    assert_eq!(snap(&s), before);
    s.redo().unwrap();
    assert_eq!(snap(&s), after);
}

#[test]
fn an_empty_group_makes_no_step_and_a_refused_command_is_not_recorded_in_it() {
    let mut s = base();
    let depth = s.undo_depth();
    s.begin_group();
    s.end_group();
    assert_eq!(s.undo_depth(), depth);
    s.begin_group();
    assert!(s.apply(&Command::RemoveNode { id: "nope".into() }).is_err());
    s.end_group();
    assert_eq!(s.undo_depth(), depth);
    s.begin_group();
    set_x(&mut s, "a", 1.0);
    assert!(s.apply(&Command::RemoveNode { id: "nope".into() }).is_err());
    s.end_group();
    assert_eq!(s.undo_depth(), depth + 1);
}

#[test]
fn a_second_begin_inside_a_group_is_ignored() {
    let mut s = base();
    let depth = s.undo_depth();
    s.begin_group();
    set_x(&mut s, "a", 1.0);
    s.begin_group();
    set_x(&mut s, "k", 2.0);
    s.end_group();
    assert_eq!(s.undo_depth(), depth + 1);
    s.undo().unwrap();
    assert_eq!((s.node("a").unwrap().x, s.node("k").unwrap().x), (0.0, 0.0));
}

#[test]
fn an_open_group_can_be_undone_and_undo_closes_it() {
    let mut s = base();
    s.begin_group();
    set_x(&mut s, "a", 9.0);
    assert!(s.can_undo());
    s.undo().unwrap();
    assert_eq!(s.node("a").unwrap().x, 0.0);
    assert_eq!(s.redo_depth(), 1);
}

#[test]
fn keyed_edits_with_the_same_key_merge_into_the_top_step() {
    let mut s = with_text_node();
    let depth = s.undo_depth();
    for t in ["h", "he", "hel", "hello"] {
        set_text(&mut s, t, Some("text:t"));
    }
    assert_eq!(s.undo_depth(), depth + 1);
    s.undo().unwrap();
    assert_eq!(s.node("t").unwrap().text, "");
    assert!(s.can_redo());
    s.redo().unwrap();
    assert_eq!(s.node("t").unwrap().text, "hello");
    set_text(&mut s, "new", None);
    assert!(!s.can_redo(), "a new edit discards redo");
}

#[test]
fn a_different_key_or_no_key_starts_a_new_step() {
    let mut s = with_text_node();
    let depth = s.undo_depth();
    set_text(&mut s, "a", Some("text:t"));
    set_text(&mut s, "ab", Some("text:other"));
    set_text(&mut s, "abc", None);
    set_text(&mut s, "abcd", Some("text:t"));
    assert_eq!(s.undo_depth(), depth + 4);
}

#[test]
fn a_keyed_edit_inside_a_group_joins_the_group() {
    let mut s = with_text_node();
    set_text(&mut s, "x", Some("text:t"));
    let depth = s.undo_depth();
    s.begin_group();
    set_text(&mut s, "xy", Some("text:t"));
    set_x(&mut s, "a", 3.0);
    s.end_group();
    assert_eq!(s.undo_depth(), depth + 1);
    s.undo().unwrap();
    assert_eq!(s.node("t").unwrap().text, "x");
    assert_eq!(s.node("a").unwrap().x, 0.0);
}

#[test]
fn a_keyed_edit_after_undo_and_redo_starts_a_new_step() {
    let mut s = with_text_node();
    set_text(&mut s, "a", Some("K"));
    s.undo().unwrap();
    s.redo().unwrap();
    let depth = s.undo_depth();
    set_text(&mut s, "ab", Some("K"));
    assert_eq!(
        s.undo_depth(),
        depth + 1,
        "a redone step is closed for merging"
    );
}

#[test]
fn a_keyed_edit_after_an_undo_does_not_merge_into_the_older_step() {
    let mut s = with_text_node();
    set_text(&mut s, "a", Some("K"));
    set_x(&mut s, "t", 5.0);
    s.undo().unwrap();
    let depth = s.undo_depth();
    set_text(&mut s, "ab", Some("K"));
    assert_eq!(s.undo_depth(), depth + 1, "the boundary the user saw stays");
}

#[test]
fn an_empty_group_still_ends_a_keyed_merge_run() {
    let mut s = with_text_node();
    set_text(&mut s, "a", Some("K"));
    let depth = s.undo_depth();
    s.begin_group();
    s.end_group();
    set_text(&mut s, "ab", Some("K"));
    assert_eq!(s.undo_depth(), depth + 1, "a group boundary closes the run");
}
