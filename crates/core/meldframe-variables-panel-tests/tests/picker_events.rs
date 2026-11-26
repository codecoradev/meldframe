//! The bind picker says each change of its state as an event: the web reads the state again only after an event, so a
//! change with none (the picker opening, its search, its closing) is never drawn, and the popover opens empty.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, BindField, Event, Panel};

mod common;
use common::*;

/// How many `PickerChanged` events the app raised since the last call.
fn changed(a: &mut App) -> usize {
    a.take_events()
        .iter()
        .filter(|e| matches!(e, Event::PickerChanged {}))
        .count()
}

fn open_picker(a: &mut App, id: &str, field: BindField) {
    a.dispatch(Action::OpenPicker {
        id: id.into(),
        field,
    });
}

fn opened() -> App {
    let mut a = open();
    select(&mut a, "plain");
    open_picker(&mut a, "plain", BindField::Fill);
    a.take_events();
    a
}

#[test]
fn opening_the_picker_is_an_event() {
    let mut a = open();
    select(&mut a, "plain");
    a.take_events();
    open_picker(&mut a, "plain", BindField::Fill);
    assert_eq!(changed(&mut a), 1);
}

#[test]
fn a_search_that_changes_the_list_is_an_event_and_the_same_search_is_none() {
    let mut a = opened();
    a.dispatch(Action::SetPickerQuery {
        query: "zzz".into(),
    });
    assert_eq!(changed(&mut a), 1);
    a.dispatch(Action::SetPickerQuery {
        query: " zzz ".into(),
    });
    assert_eq!(
        changed(&mut a),
        0,
        "the same text once trimmed is no change"
    );
    a.dispatch(Action::SetPickerQuery { query: "bg".into() });
    assert_eq!(changed(&mut a), 1);
}

#[test]
fn closing_the_picker_is_an_event_and_closing_a_closed_one_is_none() {
    let mut a = opened();
    a.dispatch(Action::ClosePicker {});
    assert_eq!(changed(&mut a), 1);
    a.dispatch(Action::ClosePicker {});
    assert_eq!(changed(&mut a), 0);
    a.dispatch(Action::SetPickerQuery { query: "bg".into() });
    assert_eq!(
        changed(&mut a),
        0,
        "a search with no picker open does nothing"
    );
}

#[test]
fn a_refused_open_closes_the_one_that_was_open_and_says_so() {
    let mut a = opened();
    open_picker(&mut a, "card", BindField::Fill);
    let events = a.take_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Refused { code } if code == "not_selected"))
    );
    assert!(
        events.iter().any(|e| matches!(e, Event::PickerChanged {})),
        "the picker that was open is gone"
    );
    open_picker(&mut a, "card", BindField::Fill);
    assert_eq!(changed(&mut a), 0, "nothing was open, nothing changed");
}

#[test]
fn the_picker_that_follows_the_modes_or_the_selection_says_so() {
    let mut a = opened();
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert_eq!(
        changed(&mut a),
        1,
        "the swatches are those of the other mode"
    );
    select(&mut a, "card");
    assert_eq!(changed(&mut a), 1, "the node is no longer the one selected");
    assert!(a.state().picker.is_none());
}

#[test]
fn binding_the_field_closes_the_picker_and_says_so() {
    let mut a = opened();
    a.dispatch(Action::BindProperty {
        id: "plain".into(),
        field: BindField::Fill,
        variable: Some("bg".into()),
    });
    assert!(a.state().picker.is_none());
    assert_eq!(changed(&mut a), 1);
}

#[test]
fn a_dispatch_that_leaves_the_picker_alone_raises_none() {
    let mut a = opened();
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: false,
    });
    assert_eq!(changed(&mut a), 0);
}
