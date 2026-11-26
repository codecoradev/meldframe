//! The Import variables dialog is open by the state of the app layer (audit finding T11, issue 324): `OpenVariablesImport`
//! opens it, `CloseVariablesImport` closes it and forgets the result, and `ClearVariablesImport` forgets the result while
//! the dialog stays open (a text that is edited, a file that is chosen, an undo that takes the import away).

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
