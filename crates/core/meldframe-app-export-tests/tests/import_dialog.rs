//! The Import variables dialog is open by the state of the app layer (audit finding T11, issue 324): `OpenVariablesImport`
//! opens it, `CloseVariablesImport` closes it and forgets the result, and `ClearVariablesImport` forgets the result while
//! the dialog stays open (a text that is edited, a file that is chosen, an undo that takes the import away).
//!
//! The name of the file the person chose enters the app layer once, as `ChooseVariablesImportSource`, and comes out made
//! safe to show and cut (rule U9, issue 416): the front end draws `variables_io.import_source` and cleans nothing.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, Connection, Event};
use meldframe_protocol::{Command, NodePatch};

const CSS: &str = ":root { --brand: #112233; }";

fn open_now(l: &Loopback) -> bool {
    l.app.state().variables_io.import_open
}

fn has_result(l: &Loopback) -> bool {
    l.app.state().variables_io.import.is_some()
}

fn changes(l: &mut Loopback) -> usize {
    let n = l
        .events
        .iter()
        .filter(|e| matches!(e, Event::VariablesIoChanged {}))
        .count();
    l.events.clear();
    n
}

#[test]
fn the_dialog_is_closed_until_it_is_opened_and_open_says_so_once() {
    let mut l = Loopback::live();
    assert!(!open_now(&l));
    l.send(Action::OpenVariablesImport {});
    assert!(open_now(&l));
    assert_eq!(changes(&mut l), 1);
    l.send(Action::OpenVariablesImport {});
    assert_eq!(changes(&mut l), 0, "open already: nothing to say");
}

#[test]
fn closing_with_a_result_closes_the_dialog_and_forgets_the_result() {
    let mut l = Loopback::live();
    l.send(Action::OpenVariablesImport {});
    l.send(Action::ImportVariables { text: CSS.into() });
    assert!(has_result(&l) && open_now(&l));
    l.send(Action::CloseVariablesImport {});
    assert!(!has_result(&l) && !open_now(&l));
    l.events.clear();
    l.send(Action::CloseVariablesImport {});
    assert_eq!(changes(&mut l), 0, "closed twice: nothing to say");
}

#[test]
fn clearing_forgets_the_result_and_leaves_the_dialog_open() {
    let mut l = Loopback::live();
    l.send(Action::OpenVariablesImport {});
    l.send(Action::ImportVariables { text: CSS.into() });
    l.events.clear();
    l.send(Action::ClearVariablesImport {});
    assert!(!has_result(&l) && open_now(&l));
    assert_eq!(changes(&mut l), 1);
    l.send(Action::ClearVariablesImport {});
    assert_eq!(changes(&mut l), 0, "no result: nothing to say");
}

#[test]
fn an_undo_that_takes_the_import_away_clears_the_result_and_keeps_the_dialog_open() {
    let mut l = Loopback::live();
    l.send(Action::OpenVariablesImport {});
    l.send(Action::ImportVariables { text: CSS.into() });
    assert!(has_result(&l));
    l.send(Action::Undo {});
    assert!(!has_result(&l), "the file no longer holds what it says");
    assert!(open_now(&l), "the person is still looking at the dialog");
}

#[test]
fn the_dialog_opens_and_closes_while_the_connection_is_halted() {
    let mut l = Loopback::live();
    l.remote(
        "agent:x",
        "b1:0",
        0,
        Command::UpdateNode {
            id: "ghost".into(),
            patch: NodePatch::default(),
        },
    );
    assert_eq!(l.app.state().connection, Connection::Halted);
    l.send(Action::OpenVariablesImport {});
    assert!(open_now(&l));
    l.send(Action::CloseVariablesImport {});
    assert!(!open_now(&l));
}

fn source(l: &Loopback) -> Option<String> {
    l.app.state().variables_io.import_source.clone()
}

fn choose(l: &mut Loopback, name: &str) {
    l.send(Action::ChooseVariablesImportSource { name: name.into() });
}

#[test]
fn a_name_with_an_override_and_a_zero_width_space_is_shown_without_them() {
    let mut l = Loopback::live();
    assert_eq!(source(&l), None);
    choose(&mut l, "th\u{202E}eme\u{200B}.css");
    assert_eq!(source(&l).as_deref(), Some("theme.css"));
}

#[test]
fn a_name_of_ten_thousand_characters_is_cut() {
    let mut l = Loopback::live();
    choose(&mut l, &"a".repeat(10_000));
    let shown = source(&l).expect("a name was chosen");
    assert_eq!(shown.chars().count(), meldframe_text::MAX_FILE_NAME);
    assert!(shown.chars().all(|c| c == 'a'));
}

#[test]
fn a_name_with_controls_loses_them_and_a_tab_is_a_space() {
    let mut l = Loopback::live();
    choose(&mut l, "a\u{7}b\u{0}\u{1b}.css");
    assert_eq!(source(&l).as_deref(), Some("ab.css"));
    choose(&mut l, "my\ttheme\n.css");
    assert_eq!(source(&l).as_deref(), Some("my theme .css"));
}

#[test]
fn a_name_that_is_nothing_is_an_empty_string_for_the_front_end_to_word() {
    let mut l = Loopback::live();
    choose(&mut l, "");
    assert_eq!(
        source(&l).as_deref(),
        Some(""),
        "a file was chosen, with no name"
    );
    choose(&mut l, "\u{202E}\u{200B}\u{2800}");
    assert_eq!(source(&l).as_deref(), Some(""));
}

#[test]
fn choosing_says_so_once_and_a_second_file_replaces_the_first() {
    let mut l = Loopback::live();
    choose(&mut l, "a.css");
    assert_eq!(changes(&mut l), 1);
    choose(&mut l, "a.css");
    assert_eq!(changes(&mut l), 0, "the same name: nothing to say");
    choose(&mut l, "b.css");
    assert_eq!(source(&l).as_deref(), Some("b.css"));
    assert_eq!(changes(&mut l), 1);
}

#[test]
fn the_name_stays_while_the_field_does_and_goes_with_it() {
    let mut l = Loopback::live();
    l.send(Action::OpenVariablesImport {});
    choose(&mut l, "theme.css");
    // Closing keeps the text of the field, so it keeps the name of its file.
    l.send(Action::CloseVariablesImport {});
    assert_eq!(source(&l).as_deref(), Some("theme.css"));
    // A refused import changes nothing in the field.
    l.send(Action::ImportVariables { text: "".into() });
    assert!(has_result(&l));
    assert_eq!(source(&l).as_deref(), Some("theme.css"));
    // Clear, a text that is edited and an import that was applied start the field again.
    l.events.clear();
    l.send(Action::ClearVariablesImport {});
    assert_eq!(source(&l), None);
    assert_eq!(changes(&mut l), 1);
    choose(&mut l, "theme.css");
    l.send(Action::ImportVariables { text: CSS.into() });
    assert_eq!(source(&l), None, "the field was emptied by the import");
}

#[test]
fn an_undo_that_takes_the_import_away_does_not_touch_the_name() {
    let mut l = Loopback::live();
    l.send(Action::ImportVariables { text: CSS.into() });
    choose(&mut l, "theme.css");
    l.send(Action::Undo {});
    assert!(!has_result(&l));
    assert_eq!(source(&l).as_deref(), Some("theme.css"));
}
