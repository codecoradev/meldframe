//! The bind picker of a field (the variable button of the Inspector board): a list of the variables that fit one field of the
//! selected node, with its own search. It exists only while it is open, and picking a row binds the field and closes it.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, BindField};
use meldframe_protocol::VarType;

mod common;
use common::*;

/// Select the node and open the picker of one of its fields: the picker belongs to the selected node.
fn open_picker(a: &mut meldframe_app::App, id: &str, field: BindField) {
    select(a, id);
    a.dispatch(Action::OpenPicker {
        id: id.into(),
        field,
    });
}

#[test]
fn there_is_no_picker_until_one_is_opened_and_none_after_it_is_closed() {
    let mut a = open();
    assert!(a.state().picker.is_none());
    open_picker(&mut a, "plain", BindField::Fill);
    assert!(a.state().picker.is_some());
    a.dispatch(Action::ClosePicker {});
    assert!(a.state().picker.is_none());
}

#[test]
fn it_lists_only_the_variables_of_the_type_of_the_field() {
    let mut a = open();
    open_picker(&mut a, "plain", BindField::Fill);
    let p = a.state().picker.clone().unwrap();
    assert_eq!((p.id.as_str(), p.field), ("plain", BindField::Fill));
    assert_eq!(
        p.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["bg"]
    );
    assert_eq!(
        (p.rows[0].collection.as_str(), p.rows[0].resolved.as_str()),
        ("appearance", "#ffffff")
    );
    open_picker(&mut a, "plain", BindField::Radius);
    let p = a.state().picker.clone().unwrap();
    assert_eq!(
        p.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["round"]
    );
}

#[test]
fn the_search_narrows_the_list_by_words_and_reports_the_total() {
    let mut a = open();
    open_picker(&mut a, "plain", BindField::Fill);
    a.dispatch(Action::SetPickerQuery {
        query: "zzz".into(),
    });
    let p = a.state().picker.clone().unwrap();
    assert_eq!((p.rows.len(), p.total, p.query.as_str()), (0, 1, "zzz"));
    a.dispatch(Action::SetPickerQuery { query: "BG".into() });
    assert_eq!(a.state().picker.as_ref().unwrap().rows.len(), 1);
}

#[test]
fn a_row_says_whether_the_field_is_bound_to_it_and_follows_the_mode() {
    let mut a = open();
    a.dispatch(Action::BindProperty {
        id: "plain".into(),
        field: BindField::Fill,
        variable: Some("bg".into()),
    });
    open_picker(&mut a, "plain", BindField::Fill);
    assert!(a.state().picker.as_ref().unwrap().rows[0].bound);
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert_eq!(
        a.state().picker.as_ref().unwrap().rows[0].resolved,
        "#000000"
    );
}

#[test]
fn binding_the_field_closes_the_picker() {
    let mut a = open();
    open_picker(&mut a, "plain", BindField::Fill);
    a.dispatch(Action::BindProperty {
        id: "plain".into(),
        field: BindField::Fill,
        variable: Some("bg".into()),
    });
    assert!(a.state().picker.is_none());
    assert_eq!(binds(&a, "bg")[0].field, BindField::Fill);
}

#[test]
fn a_bind_that_is_refused_leaves_the_picker_open_and_the_field_as_it_was() {
    let mut a = open();
    open_picker(&mut a, "plain", BindField::Fill);
    a.dispatch(Action::BindProperty {
        id: "plain".into(),
        field: BindField::Fill,
        variable: Some("ghost".into()),
    });
    assert_eq!(refused(&mut a), vec!["E_VARIABLE_NOT_FOUND"]);
    assert!(a.state().picker.is_some());
    assert!(binds(&a, "bg").iter().all(|b| !b.bound));
}

#[test]
fn the_picker_says_the_type_of_variable_it_lists() {
    let mut a = open();
    open_picker(&mut a, "plain", BindField::Fill);
    assert_eq!(a.state().picker.as_ref().unwrap().kind, VarType::Color);
    open_picker(&mut a, "plain", BindField::Opacity);
    assert_eq!(a.state().picker.as_ref().unwrap().kind, VarType::Number);
}

#[test]
fn it_closes_when_the_selection_moves_off_the_node() {
    let mut a = open();
    open_picker(&mut a, "plain", BindField::Fill);
    select(&mut a, "card");
    assert!(a.state().picker.is_none());
}

#[test]
fn a_field_the_node_cannot_hold_a_missing_node_or_a_node_that_is_not_selected_is_refused() {
    let mut a = open();
    open_picker(&mut a, "label", BindField::Radius);
    assert_eq!(refused(&mut a), vec!["not_bindable"]);
    open_picker(&mut a, "nobody", BindField::Fill);
    assert_eq!(refused(&mut a), vec!["not_found"]);
    select(&mut a, "plain");
    a.dispatch(Action::OpenPicker {
        id: "card".into(),
        field: BindField::Fill,
    });
    assert_eq!(refused(&mut a), vec!["not_selected"]);
    assert!(a.state().picker.is_none());
}

#[test]
fn it_is_not_built_while_closed() {
    let mut a = open();
    let b = a.picker_builds();
    a.dispatch(Action::Select {
        ids: vec!["plain".into()],
    });
    a.dispatch(Action::Move {
        id: "plain".into(),
        x: 5.0,
        y: 5.0,
    });
    assert_eq!(a.picker_builds(), b);
    open_picker(&mut a, "plain", BindField::Fill);
    assert!(a.picker_builds() > b);
}
