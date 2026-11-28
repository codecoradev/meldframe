//! A double click in the pointer machine: two clicks on the same node close in time and place begin a text edit on it, by the
//! clock the front end reports (`PointerInput::at_ms`; the app layer reads none), against the made-up canvas of `pointer_rig`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod pointer_rig;
use meldframe_app_model::Camera;
use pointer_rig::*;
use serde_json::json;

fn begin(id: &str) -> serde_json::Value {
    json!({"type": "textEdit", "op": {"type": "begin", "id": id}})
}

/// A click at `(x, y)` at `ms`: a press and a release.
fn click(r: &mut Rig, ms: Option<u64>, x: f64, y: f64) {
    r.now = ms;
    r.down(x, y);
    r.up(x, y);
}

fn edits(r: &Rig) -> Vec<serde_json::Value> {
    r.json()
        .into_iter()
        .filter(|a| a["type"] == "textEdit")
        .collect()
}

#[test]
fn two_clicks_on_one_node_close_in_time_and_place_begin_a_text_edit() {
    let mut r = rig(Camera::default(), &[]);
    click(&mut r, Some(1000), 50.0, 50.0);
    assert!(edits(&r).is_empty(), "one click is a click");
    click(&mut r, Some(1300), 52.0, 51.0);
    assert_eq!(edits(&r), [begin("a")]);
}

#[test]
fn too_late_too_far_no_clock_or_a_clock_that_went_back_is_no_double_click() {
    let cases: [(&str, Option<u64>, (f64, f64)); 5] = [
        ("501 ms is too late", Some(1501), (50.0, 50.0)),
        ("5 px is too far", Some(1100), (55.0, 50.0)),
        ("another place", Some(1100), (250.0, 50.0)),
        ("no clock", None, (50.0, 50.0)),
        ("the clock went back", Some(900), (50.0, 50.0)),
    ];
    for (why, ms, at) in cases {
        let mut r = rig(Camera::default(), &[]);
        click(&mut r, Some(1000), 50.0, 50.0);
        click(&mut r, ms, at.0, at.1);
        assert!(edits(&r).is_empty(), "{why}");
    }
    // The edge: 500 ms and 4 px are still a double click.
    let mut r = rig(Camera::default(), &[]);
    click(&mut r, Some(1000), 50.0, 50.0);
    click(&mut r, Some(1500), 54.0, 50.0);
    assert_eq!(edits(&r), [begin("a")]);
}

#[test]
fn a_drag_a_cancel_or_a_click_on_nothing_between_the_clicks_starts_the_count_again() {
    let mut r = rig(Camera::default(), &[]);
    click(&mut r, Some(1000), 50.0, 50.0);
    r.now = Some(1100);
    r.down(50.0, 50.0);
    r.mv(80.0, 50.0);
    r.up(80.0, 50.0);
    click(&mut r, Some(1200), 50.0, 50.0);
    assert!(edits(&r).is_empty(), "the drag in between");
    let mut c = rig(Camera::default(), &[]);
    click(&mut c, Some(1000), 50.0, 50.0);
    c.send(meldframe_app_model::PointerPhase::Cancel, 50.0, 50.0, false);
    click(&mut c, Some(1100), 50.0, 50.0);
    assert!(edits(&c).is_empty(), "the cancel in between");
    let mut e = rig(Camera::default(), &[]);
    click(&mut e, Some(1000), 50.0, 50.0);
    click(&mut e, Some(1100), 500.0, 500.0);
    click(&mut e, Some(1200), 50.0, 50.0);
    assert!(edits(&e).is_empty(), "a click on nothing in between");
}

#[test]
fn a_third_click_is_not_another_double_click_and_a_fourth_is() {
    let mut r = rig(Camera::default(), &[]);
    for ms in [1000, 1100, 1200] {
        click(&mut r, Some(ms), 50.0, 50.0);
    }
    assert_eq!(edits(&r), [begin("a")], "one for the first two");
    click(&mut r, Some(1300), 50.0, 50.0);
    assert_eq!(
        edits(&r),
        [begin("a"), begin("a")],
        "the third and the fourth"
    );
}

#[test]
fn a_press_is_busy_from_the_press_to_the_release_or_the_cancel() {
    let mut r = rig(Camera::default(), &[]);
    assert!(!r.machine.busy());
    r.mv(50.0, 50.0);
    assert!(!r.machine.busy(), "a hover is not a press");
    r.down(50.0, 50.0);
    assert!(r.machine.busy(), "before the drag has begun too");
    r.up(50.0, 50.0);
    assert!(!r.machine.busy());
    r.down(500.0, 500.0);
    r.send(
        meldframe_app_model::PointerPhase::Cancel,
        500.0,
        500.0,
        false,
    );
    assert!(!r.machine.busy());
}
