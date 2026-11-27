//! Whether the manage view is open is state of the app layer (audit finding T11, issue 324): `ManageOp::Open` opens it and
//! clears the search of the panel (the view draws the rows of the panel), `ManageOp::Close` closes it and forgets what it held.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, Connection, Event, ManageOp};
use meldframe_protocol::{Command, NodePatch};
mod common;
use common::*;

fn query(a: &App) -> String {
    a.state().variables.query.clone()
}

fn search(a: &mut App, text: &str) {
    a.dispatch(Action::SetVariablesQuery { query: text.into() });
}

#[test]
fn the_view_is_closed_until_it_is_opened() {
    let mut a = open();
    assert!(!a.state().manage.open);
    a.dispatch(op(ManageOp::Open {}));
    assert!(a.state().manage.open);
    assert!(
        a.take_events().contains(&Event::ManageChanged {}),
        "a redraw is asked for"
    );
}

#[test]
fn open_manage_clears_the_search() {
    let mut a = open();
    search(&mut a, "bg");
    assert_eq!(query(&a), "bg");
    a.take_events();
    a.dispatch(op(ManageOp::Open {}));
    assert_eq!(query(&a), "");
    let events = a.take_events();
    assert!(
        events.contains(&Event::VariablesQueryChanged { query: "".into() }),
        "{events:?}"
    );
}

#[test]
fn opening_what_is_open_says_nothing_and_keeps_the_search_the_person_typed_since() {
    let mut a = open();
    a.dispatch(op(ManageOp::Open {}));
    search(&mut a, "bg");
    a.take_events();
    a.dispatch(op(ManageOp::Open {}));
    assert_eq!(query(&a), "bg");
    assert!(a.take_events().is_empty());
}

#[test]
fn closing_closes_the_view_and_forgets_what_it_held() {
    let mut a = open();
    a.dispatch(op(ManageOp::Open {}));
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("bg"),
    }));
    a.dispatch(op(ManageOp::Close {}));
    let m = &a.state().manage;
    assert!(!m.open && m.removal.is_none());
    // Opened again, the view starts clean.
    a.dispatch(op(ManageOp::Open {}));
    assert!(a.state().manage.open && a.state().manage.removal.is_none());
}

#[test]
fn the_view_opens_and_closes_while_the_connection_is_halted() {
    let mut a = open();
    remote(
        &mut a,
        1,
        Command::UpdateNode {
            id: "ghost".into(),
            patch: NodePatch::default(),
        },
    );
    assert_eq!(a.state().connection, Connection::Halted);
    a.dispatch(op(ManageOp::Open {}));
    assert!(a.state().manage.open);
    a.dispatch(op(ManageOp::Close {}));
    assert!(!a.state().manage.open);
}
