//! The text of a value field of the Variables panel (`TypedText::VariableValue` of `Action::Typed`, audit finding T4): empty text sends
//! nothing, any other text goes to the core as a literal, and the core says what it comes to. These are the edges the
//! front end used to decide for itself: the same value, a literal over an alias, a bad number and a mode the row lacks.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, TypedText, VariableValue};
use serde_json::json;

mod common;
use common::*;

fn typed(a: &mut App, variable: &str, mode: &str, text: &str) {
    a.dispatch(Action::Typed {
        entry: TypedText::VariableValue {
            variable: variable.into(),
            mode: mode.into(),
            text: text.into(),
        },
    });
}

#[test]
fn a_text_that_is_the_value_already_makes_no_step_and_no_notice() {
    let mut a = open();
    typed(&mut a, "bg", "light", "#ffffff");
    typed(&mut a, "bg", "light", "  #ffffff  ");
    typed(&mut a, "round", "light", "4");
    assert_eq!(a.state().history.undo_steps, 0);
    assert!(a.take_events().is_empty());
}

#[test]
fn a_text_changes_the_literal_as_one_undo_step_with_the_spaces_around_it_left_out() {
    let mut a = open();
    typed(&mut a, "round", "dark", " 8.5 ");
    assert!(refused(&mut a).is_empty());
    assert_eq!(texts(row(&a, "round"))[1], ("dark", "8.5"));
    assert_eq!(a.state().history.undo_steps, 1);
    a.dispatch(Action::Undo {});
    assert_eq!(texts(row(&a, "round"))[1], ("dark", "12"));
}

#[test]
fn a_literal_typed_over_an_alias_makes_the_value_a_literal() {
    let mut a = open();
    a.dispatch(Action::EditVariables {
        commands: vec![cmd(
            json!({"type": "addVariable", "variable": {"id": "panel", "collection": "t", "name": "color/panel", "type": "color",
            "values": {"light": "#111111", "dark": "#222222"}}}),
        )],
    });
    a.dispatch(Action::SetVariableValue {
        variable: "panel".into(),
        mode: "dark".into(),
        value: VariableValue::Alias {
            variable: "bg".into(),
        },
    });
    assert_eq!(row(&a, "panel").values[1].alias.as_deref(), Some("bg"));
    let steps = a.state().history.undo_steps;
    typed(&mut a, "panel", "dark", "#abcdef");
    assert!(refused(&mut a).is_empty());
    let v = &row(&a, "panel").values[1];
    assert_eq!((v.alias.as_deref(), v.text.as_str()), (None, "#abcdef"));
    assert_eq!(a.state().history.undo_steps, steps + 1);
}

#[test]
fn a_text_that_is_not_a_finite_number_for_a_number_variable_is_refused_and_changes_nothing() {
    let mut a = open();
    for text in ["wide", "1,5", "0x10"] {
        typed(&mut a, "round", "light", text);
        assert_eq!(refused(&mut a), ["invalid_value"], "{text}");
    }
    // Rust reads these as numbers that are not finite, and the core refuses them in its own words.
    for text in ["NaN", "inf", "1e999"] {
        typed(&mut a, "round", "light", text);
        assert_eq!(refused(&mut a), ["E_NOT_FINITE"], "{text}");
    }
    assert_eq!(a.state().history.undo_steps, 0);
    assert_eq!(texts(row(&a, "round"))[0], ("light", "4"));
}

#[test]
fn a_mode_the_row_does_not_have_is_refused_by_the_core() {
    let mut a = open();
    typed(&mut a, "bg", "nope", "#fff");
    assert_eq!(refused(&mut a), ["E_MODE_NOT_FOUND"]);
    typed(&mut a, "nope", "light", "#fff");
    assert_eq!(refused(&mut a), ["E_VARIABLE_NOT_FOUND"]);
    assert_eq!(a.state().history.undo_steps, 0);
}

#[test]
fn an_empty_text_sends_nothing_even_for_a_variable_or_mode_that_does_not_exist() {
    let mut a = open();
    typed(&mut a, "nope", "nope", "   ");
    assert!(a.take_events().is_empty());
}
