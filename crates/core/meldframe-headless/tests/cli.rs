//! The command line entry: the scenario script runs as a process, and the exit code says whether it held. Native only (a
//! process is not started under WASM).
#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_meldframe-headless"))
        .args(args)
        .output()
        .expect("the binary starts")
}

fn script(name: &str) -> String {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/scripts");
    dir.join(name).to_string_lossy().into_owned()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn the_scenario_script_exits_zero_and_prints_what_it_saw() {
    let o = run(&["--strict", &script("scenario.script")]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let out = text(&o.stdout);
    assert!(out.contains("opened nodes.json: 3 nodes"), "{out}");
    assert!(out.contains("effects: raster finishRaster"), "{out}");
    assert!(out.contains("  confirmed seq: 4"), "{out}");
}

#[test]
fn the_pointer_flow_and_the_capture_scripts_exit_zero_as_processes_too() {
    for name in ["pointer_flow.script", "capture.script"] {
        let o = run(&["--strict", &script(name)]);
        assert!(o.status.success(), "{name}: {}", text(&o.stderr));
    }
}

#[test]
fn a_refused_step_exits_one_with_strict_and_zero_without() {
    let o = run(&["--strict", &script("refused.script")]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stderr).contains("line 2: refused: not_found"),
        "{}",
        text(&o.stderr)
    );
    let o = run(&[&script("refused.script")]);
    assert_eq!(o.status.code(), Some(0));
    assert!(text(&o.stdout).contains("  refused: not_found"));
}

#[test]
fn a_strict_run_lets_an_expected_refusal_through() {
    let o = run(&["--strict", &script("refused_expected.script")]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(text(&o.stdout).contains("  refused: not_found"));
}

#[test]
fn a_script_that_cannot_be_read_and_a_missing_argument_exit_two() {
    assert_eq!(run(&[&script("none.script")]).status.code(), Some(2));
    assert_eq!(run(&[]).status.code(), Some(2));
}
