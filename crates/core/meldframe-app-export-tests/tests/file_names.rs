//! The file name of an export (#319, finding T1 of the thin-UI audit): the app layer works it out, from the frame's name,
//! so a front end only adds the scale and the extension it knows. It is the same decision for the PNG and the HTML, made
//! with the strict profile of `meldframe-text` (a file name is identifier-like) so that a name with a hidden character
//! reaches no file name: the web client's own version dropped some bidirectional controls and kept the zero width space.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App};
use serde_json::json;

/// The target's stem and the HTML file of the dialog, for a page whose one frame has this name and this id.
fn names(name: &str, id: &str) -> (String, String) {
    let nodes = serde_json::from_value(
        json!([{"id": id, "type": "frame", "parentId": null, "name": name, "w": 100, "h": 100}]),
    )
    .unwrap();
    let mut app = App::new("user:me", "c0", 1, nodes, 0).unwrap();
    app.dispatch(Action::Select {
        ids: vec![id.to_string()],
    });
    let stem = app.state().export.target.clone().unwrap().file_stem;
    app.dispatch(Action::OpenExport {});
    let html = app.state().export.dialog.clone().unwrap().html_file;
    (stem, html)
}

fn stem(name: &str) -> String {
    names(name, "n1").0
}

#[test]
fn a_plain_name_is_the_file_name() {
    assert_eq!(
        names("Landing Desktop", "n1"),
        ("Landing Desktop".into(), "Landing Desktop.html".into())
    );
    assert_eq!(stem("Hero"), "Hero");
}

#[test]
fn a_hidden_character_reaches_no_file_name() {
    // The golden table of #319: braille blank, Hangul filler, Khmer vowel, zero width space, soft hyphen, a no-break
    // space (two of them are one space), and a right-to-left override.
    for (c, want) in [
        ("\u{2800}", "ab"),
        ("\u{3164}", "ab"),
        ("\u{17B4}", "ab"),
        ("\u{200B}", "ab"),
        ("\u{00AD}", "ab"),
        ("\u{00A0}\u{00A0}", "a b"),
        ("\u{202E}", "ab"),
    ] {
        assert_eq!(stem(&format!("a{c}b")), want, "{c:?}");
    }
    // A name cannot be made to read as another extension.
    assert_eq!(stem("photo\u{202E}gnp.exe"), "photognp.exe");
    assert_eq!(names("a\u{200B}b", "n1").1, "ab.html");
}

#[test]
fn control_characters_and_the_characters_a_file_system_refuses_become_hyphens() {
    assert_eq!(stem("a/b\\c:d*e?f\"g<h>i|j"), "a-b-c-d-e-f-g-h-i-j");
    assert_eq!(stem("tab\there"), "tab-here");
    assert_eq!(stem("a\u{0}b\u{1f}c\u{7f}d"), "a-b-c-d");
}

#[test]
fn leading_dots_and_spaces_go_and_the_name_is_cut_at_80_characters() {
    assert_eq!(stem("..hidden"), "hidden");
    assert_eq!(stem("  . x"), "x");
    assert_eq!(stem(&"x".repeat(200)), "x".repeat(80));
    assert_eq!(stem(&"\u{1F600}".repeat(100)), "\u{1F600}".repeat(80));
}

#[test]
fn a_name_that_is_nothing_falls_back_to_the_id_and_then_to_export() {
    assert_eq!(names("   ", "frame-7").0, "frame-7");
    assert_eq!(names("\u{202E}\u{200B}", "frame-7").0, "frame-7");
    assert_eq!(names("", "x").0, "x");
}

#[test]
fn the_names_windows_refuses_get_a_hyphen() {
    assert_eq!(stem("CON"), "CON-");
    assert_eq!(stem("nul"), "nul-");
    assert_eq!(stem("Console"), "Console");
    assert_eq!(stem("NUL.tar"), "NUL-.tar");
    assert_eq!(stem("con.backup.v2"), "con-.backup.v2");
    assert_eq!(stem("com1"), "com1-");
    assert_eq!(stem("com0"), "com0");
}
