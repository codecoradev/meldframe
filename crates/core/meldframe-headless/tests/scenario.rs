//! The scenario of E8.1.9 through the harness, which is a front end with no window: open a file, select a frame, move it
//! (an undo puts it back), resize a rectangle by a handle, press keys, run a palette entry, export a frame as a PNG and
//! read the state. Every number is the app layer's answer; the harness only sends actions and answers effects.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::{SCENARIO, files, go, node};
use meldframe_headless::run;
use serde_json::json;

#[test]
fn the_whole_scenario_goes_through_with_every_check_passing() {
    let r = run(SCENARIO, &files, true);
    assert_eq!(r.failure, None, "{}", r.out);
    assert!(r.refusals.is_empty(), "{:?}", r.refusals);
    let s = r.shell.expect("a file is open");
    assert_eq!(s.state()["confirmedSeq"], 6);
    assert_eq!(s.delivered(), Some((600, 400)));
}

#[test]
fn a_move_is_one_undo_step_and_undo_puts_the_frame_back() {
    let r = go("select f1\nmove f1 150 160", true);
    assert_eq!(r.failure, None, "{}", r.out);
    assert_eq!(
        (node(&r, "f1")["x"].clone(), node(&r, "f1")["y"].clone()),
        (json!(150.0), json!(160.0))
    );
    let r = go("select f1\nmove f1 150 160\nundo", true);
    assert_eq!(
        (node(&r, "f1")["x"].clone(), node(&r, "f1")["y"].clone()),
        (json!(100.0), json!(100.0))
    );
    // The two steps went to the made-up server and came back confirmed.
    let s = r.shell.unwrap();
    assert_eq!(s.state()["confirmedSeq"], 2);
    assert_eq!(s.state()["pending"], json!([]));
}

#[test]
fn a_drop_moves_by_the_travel_and_a_handle_changes_one_edge() {
    let r = go("drop f1 300 300 40 -20\nhandle r1 nw 4 6", true);
    assert_eq!(r.failure, None, "{}", r.out);
    assert_eq!(
        (node(&r, "f1")["x"].clone(), node(&r, "f1")["y"].clone()),
        (json!(140.0), json!(80.0))
    );
    // The north west handle moves the left and top edges and the opposite corner stays.
    let b = node(&r, "r1");
    assert_eq!(
        [&b["x"], &b["y"], &b["w"], &b["h"]],
        [&json!(14.0), &json!(16.0), &json!(46.0), &json!(34.0)]
    );
}

#[test]
fn resize_and_draw_are_actions_of_the_app_and_the_new_node_is_selected() {
    let r = go("resize r1 120 90\ndraw rectangle 120 120 180 160", true);
    assert_eq!(r.failure, None, "{}", r.out);
    assert_eq!(
        (node(&r, "r1")["w"].clone(), node(&r, "r1")["h"].clone()),
        (json!(120.0), json!(90.0))
    );
    let s = r.shell.unwrap();
    assert_eq!(s.app().nodes().len(), 4);
    assert_eq!(s.state()["selection"].as_array().map(Vec::len), Some(1));
    assert_eq!(s.state()["tool"], "select");
}

#[test]
fn keys_are_read_by_the_app_layer_not_the_harness() {
    let r = go(
        "key e\nexpect state /tool \"ellipse\"\nkey Escape\nkey shift+?\nexpect state /overlay \"help\"\nkey ctrl+k\nexpect state /overlay \"palette\"",
        true,
    );
    assert_eq!(r.failure, None, "{}", r.out);
}

#[test]
fn the_camera_is_view_state_and_writes_nothing_to_the_file() {
    let r = go(
        "camera viewport 800 600\ncamera pan 30 40\nexpect state /camera/x 30\nexpect state /camera/y 40\ncamera zoom in",
        true,
    );
    assert_eq!(r.failure, None, "{}", r.out);
    let s = r.shell.unwrap();
    assert_eq!(s.state()["viewport"], json!({"w": 800.0, "h": 600.0}));
    assert!(s.state()["camera"]["zoom"].as_f64().unwrap() > 1.0);
    // Nothing was sent to the file.
    assert_eq!(s.state()["confirmedSeq"], 0);
}

#[test]
fn the_palette_lists_entries_and_runs_one_by_its_id() {
    let r = go(
        "palette undo\nselect r1\nmove r1 20 20\npalette undo run undo\nexpect node r1 x 10",
        true,
    );
    assert_eq!(r.failure, None, "{}", r.out);
    assert!(r.out.contains("  undo: Undo"), "{}", r.out);
}

#[test]
fn a_png_export_is_planned_by_the_app_and_drawn_by_the_shell() {
    let r = go(
        "export f1 png 1\nexpect delivered 300 200\nexport f1 png 0.5\nexpect delivered 150 100",
        true,
    );
    assert_eq!(r.failure, None, "{}", r.out);
    // The app asked for the drawing, then for its delivery: the harness drew nothing and answered both.
    assert!(r.out.contains("effects: raster finishRaster"), "{}", r.out);
}

#[test]
fn an_html_export_reports_the_file_the_app_planned() {
    let r = go("export f1 html", true);
    assert_eq!(r.failure, None, "{}", r.out);
    assert!(r.out.contains("html: \"Card.html\""), "{}", r.out);
}

#[test]
fn every_pointer_flag_reaches_the_app_as_part_of_the_input() {
    let r = go(
        "pointer move 5 5 shift ctrl alt meta space touch middle",
        true,
    );
    assert_eq!(r.failure, None, "{}", r.out);
}
