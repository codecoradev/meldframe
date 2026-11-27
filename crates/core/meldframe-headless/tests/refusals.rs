//! A step the app refuses, a check that does not hold and a line that is not a step make the harness fail, so a script is
//! a test of the app layer from outside.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::{files, go};
use meldframe_headless::run;

#[test]
fn a_refused_step_is_listed_and_the_run_goes_on_when_it_is_not_strict() {
    let r = go("move nope 1 2\nmove f1 5 5", false);
    assert_eq!(r.failure, None, "{}", r.out);
    assert_eq!(r.refusals, ["not_found"]);
    assert!(r.out.contains("  refused: not_found"), "{}", r.out);
}

#[test]
fn a_refused_step_fails_a_strict_run_at_its_line_and_stops() {
    let r = go("move nope 1 2\nmove f1 5 5", true);
    assert_eq!(r.failure.as_deref(), Some("line 2: refused: not_found"));
    // The step after it did not run.
    let f1 = &r.shell.unwrap().nodes()[1];
    assert_eq!(f1["x"], 100.0);
}

#[test]
fn the_strict_step_turns_it_on_from_its_line() {
    let r = go("move nope 1 2\nstrict\nmove nope 1 2", false);
    assert_eq!(r.failure.as_deref(), Some("line 4: refused: not_found"));
    assert_eq!(r.refusals.len(), 2);
}

#[test]
fn a_refusal_can_be_expected() {
    let r = go("export r1 png\nexpect refused not_exportable", false);
    assert_eq!(r.failure, None, "{}", r.out);
    let r = go("move f1 1 1\nexpect refused not_found", false);
    assert!(
        r.failure
            .unwrap()
            .starts_with("line 3: expected refused not_found")
    );
}

#[test]
fn a_palette_entry_the_app_refuses_is_a_refusal() {
    let r = go("palette x run no-such-entry", true);
    assert_eq!(
        r.failure.as_deref(),
        Some("line 2: refused: palette_refused")
    );
}

#[test]
fn a_check_that_does_not_hold_fails_the_run_in_any_mode() {
    let r = go("move f1 150 160\nexpect node f1 x 151", false);
    assert_eq!(
        r.failure.as_deref(),
        Some("line 3: node f1 x: expected 151, got 150.0")
    );
    let r = go("expect state /tool \"text\"", false);
    assert_eq!(
        r.failure.as_deref(),
        Some("line 2: state /tool: expected \"text\", got \"select\"")
    );
    let r = go("expect delivered 1 1", false);
    assert!(
        r.failure
            .unwrap()
            .starts_with("line 2: expected an image of 1 by 1")
    );
}

#[test]
fn a_line_that_is_not_a_step_names_its_line_and_why() {
    for (line, why) in [
        ("jump f1", "unknown step: jump"),
        ("move f1 one 2", "x is not a number: one"),
        ("move f1 1", "missing y"),
        ("key hyper+x", "unknown modifier: hyper"),
        ("handle r1 up 1 1", "not an action:"),
        ("camera tilt", "unknown camera step: tilt"),
        ("export f1 gif", "unknown export form: gif"),
        ("pointer sideways 1 1", "not an action:"),
        ("pointer down 1 1 hyper", "unknown pointer flag: hyper"),
    ] {
        let r = go(line, false);
        let failure = r.failure.unwrap_or_default();
        assert!(
            failure.starts_with("line 2: ") && failure.contains(why),
            "{line}: {failure}"
        );
    }
}

#[test]
fn a_step_before_a_file_is_open_and_a_file_that_is_not_there_fail() {
    let r = run("select f1", &files, false);
    assert_eq!(r.failure.as_deref(), Some("line 1: no file is open"));
    let r = run("open other.json", &files, false);
    assert_eq!(
        r.failure.as_deref(),
        Some("line 1: other.json: no such file")
    );
    let r = run("# only a comment\n\n", &files, true);
    assert_eq!(r.failure, None);
}

#[test]
fn a_strict_run_holds_a_refusal_for_the_next_step_and_an_expect_consumes_it() {
    let r = go("move nope 1 2\nexpect refused not_found\nmove f1 5 5", true);
    assert_eq!(r.failure, None, "{}", r.out);
    assert_eq!(r.refusals, ["not_found"]);
    assert_eq!(r.shell.unwrap().nodes()[1]["x"], 5.0);
}

#[test]
fn a_held_refusal_fails_at_its_own_line_when_the_next_step_is_not_an_expect() {
    let r = go("move nope 1 2\nstate", true);
    assert_eq!(r.failure.as_deref(), Some("line 2: refused: not_found"));
    // The step after it did not run.
    assert!(!r.out.contains("[3] state"), "{}", r.out);
    // Nor does the end of the script let it go.
    let r = go("move nope 1 2", true);
    assert_eq!(r.failure.as_deref(), Some("line 2: refused: not_found"));
}

#[test]
fn an_expect_for_another_code_does_not_excuse_a_refusal() {
    let r = go("move nope 1 2\nexpect refused invalid", true);
    assert!(
        r.failure
            .unwrap()
            .starts_with("line 3: expected refused invalid")
    );
}

#[test]
fn a_scale_the_app_refuses_stops_the_export_instead_of_exporting_at_the_old_scale() {
    let r = go(
        "export f1 png 7\nexpect refused invalid_scale\nexpect state /export/png/scale 1",
        true,
    );
    assert_eq!(r.failure, None, "{}", r.out);
    assert!(!r.out.contains("raster"), "{}", r.out);
    assert_eq!(r.shell.unwrap().delivered(), None);
}
