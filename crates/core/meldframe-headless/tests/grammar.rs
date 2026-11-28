//! The four lines the pointer flow and the capture path need beyond the grammar of the first scenario: `expect preview` (what the
//! gesture going on would do, read from `App::pointer_preview`), `expect order` (the children of a node in sibling order),
//! `expect effect` (an effect the last step returned) and `bytes` (how large the shell says its drawings are). Each is checked
//! on a case that holds and on one that does not, so a check that always passed would be found.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::go;

#[test]
fn a_preview_is_read_while_a_gesture_goes_on_and_is_none_once_it_ended() {
    let r = go(
        "pointer down 120 120\npointer move 150 140\npointer move 170 150\n\
         expect preview /kind \"move\"\nexpect preview /dx 50\nexpect preview /dy 30\n\
         pointer up 170 150\nexpect preview /kind null",
        true,
    );
    assert_eq!(r.failure, None, "{}", r.out);
    let r = go(
        "pointer down 120 120\npointer move 150 140\npointer move 170 150\nexpect preview /dx 51",
        true,
    );
    assert_eq!(
        r.failure.as_deref(),
        Some("line 5: preview /dx: expected 51, got 50.0")
    );
    // Nothing is going on, so there is no preview to be a move.
    let r = go("expect preview /kind \"move\"", true);
    assert_eq!(
        r.failure.as_deref(),
        Some("line 2: preview /kind: expected \"move\", got null")
    );
}

#[test]
fn the_children_of_a_node_are_checked_in_sibling_order() {
    let r = go("expect order root f1\nexpect order f1 r1", true);
    assert_eq!(r.failure, None, "{}", r.out);
    let r = go("expect order f1 r1 r2", true);
    assert_eq!(
        r.failure.as_deref(),
        Some("line 2: order of f1: expected [\"r1\", \"r2\"], got [\"r1\"]")
    );
    let r = go("expect order nowhere", true);
    assert_eq!(
        r.failure, None,
        "a node with no children has none: {}",
        r.out
    );
}

#[test]
fn an_effect_of_the_last_step_is_checked_by_its_kind_and_a_field() {
    let r = go(
        "export f1 png 1\nexpect effect raster\nexpect effect raster width 300\nexpect effect finishRaster deliver true",
        true,
    );
    assert_eq!(r.failure, None, "{}", r.out);
    let r = go("export f1 png 1\nexpect effect raster width 301", true);
    assert_eq!(
        r.failure.as_deref(),
        Some(
            "line 3: no raster effect with width 301 in the last step: [\"raster\", \"finishRaster\"]"
        )
    );
    let r = go("select f1\nexpect effect raster", true);
    assert_eq!(
        r.failure.as_deref(),
        Some("line 3: no raster effect in the last step: []")
    );
}

#[test]
fn the_bytes_the_shell_reports_for_a_drawing_can_be_set_and_must_be_a_size() {
    let r = go("bytes 100\nexport f1 png 1\nexpect delivered 300 200", true);
    assert_eq!(r.failure, None, "{}", r.out);
    let r = go("bytes many", false);
    assert_eq!(
        r.failure.as_deref(),
        Some("line 2: bytes is not a size: many")
    );
    let r = go("bytes", false);
    assert_eq!(r.failure.as_deref(), Some("line 2: missing a size"));
}
