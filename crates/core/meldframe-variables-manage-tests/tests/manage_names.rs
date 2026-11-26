//! The checks of a name in the Variables manage view, as app state (audit finding T5, issue 322): the rule of the core, the
//! limits, the names a new thing is compared with and what the forms start again with. The front end draws
//! `AppState::manage` and sends `Action::Manage`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, ManageOp, NameError, NameWarning, VarType, VariableEntity};

mod common;
use common::*;

#[test]
fn the_limits_are_the_cores() {
    let a = open();
    let l = a.state().manage.limits;
    let core = meldframe_check::Limits::default();
    assert_eq!(
        (l.name_max, l.collections, l.modes, l.variables),
        (
            core.max_name,
            core.max_collections,
            core.max_modes,
            core.max_variables
        )
    );
    // A mode is a short label: the view's own ceiling, under the core's.
    assert_eq!(l.mode_name_max, meldframe_app::MODE_NAME_MAX);
    assert!(l.mode_name_max <= core.max_name);
}

#[test]
fn a_name_is_accepted_when_it_hides_nothing_even_if_it_is_an_emoji_with_a_selector() {
    let mut a = open();
    // The sun with variation selector 16, a family joined by zero-width joiners, a keycap and a flag.
    for text in [
        "\u{2600}\u{fe0f}",
        "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}",
        "1\u{fe0f}\u{20e3}",
        "\u{1f1e9}\u{1f1ea}",
        "High Contrast \u{2600}\u{fe0f}",
    ] {
        typed(&mut a, VariableEntity::Mode, text);
        assert_eq!(check(&a, VariableEntity::Mode).0, None, "{text:?}");
    }
    // The other selectors and the joiner that joins nothing hide a second meaning and are refused.
    for text in [
        "a\u{fe00}b",
        "a\u{fe0f}b",
        "a\u{200d}b",
        "a\u{e0100}b",
        "a\u{180b}b",
    ] {
        typed(&mut a, VariableEntity::Mode, text);
        assert_eq!(
            check(&a, VariableEntity::Mode).0,
            Some(NameError::Hidden),
            "{text:?}"
        );
    }
    for text in [
        "a\u{feff}b",
        "a\u{202e}b",
        "a\u{200b}b",
        "a\u{a0}b",
        "a\tb",
        "a\nb",
    ] {
        typed(&mut a, VariableEntity::Mode, text);
        assert_eq!(
            check(&a, VariableEntity::Mode).0,
            Some(NameError::Hidden),
            "{text:?}"
        );
    }
}

#[test]
fn check_name_variable_rejects_uppercase_and_double_hyphen() {
    let mut a = open();
    for (text, want) in [
        ("Color/Brand", Some(NameError::NotPath)),
        ("color--brand", Some(NameError::NotPath)),
        ("color/-brand", Some(NameError::NotPath)),
        ("color/brand-", Some(NameError::NotPath)),
        ("color//brand", Some(NameError::NotPath)),
        ("colour brand", Some(NameError::NotPath)),
        ("", Some(NameError::Empty)),
        ("   ", Some(NameError::Empty)),
        ("color/brand-500", None),
        ("  color/brand-500  ", None),
    ] {
        typed(&mut a, VariableEntity::Variable, text);
        let (error, _, name) = check(&a, VariableEntity::Variable);
        assert_eq!(error, want, "{text:?}");
        assert_eq!(
            name,
            text.trim(),
            "the name to send has no spaces around it"
        );
    }
}

#[test]
fn a_name_with_a_hidden_character_or_a_space_that_is_not_plain_is_refused() {
    let mut a = open();
    for text in [
        "a\u{202e}b",
        "a\u{200b}b",
        "a\u{a0}b",
        "a\u{3000}b",
        "a\tb",
        "a\u{115f}b",
    ] {
        for form in [
            VariableEntity::Variable,
            VariableEntity::Collection,
            VariableEntity::Mode,
        ] {
            typed(&mut a, form, text);
            assert_eq!(check(&a, form).0, Some(NameError::Hidden), "{text:?}");
        }
    }
}

#[test]
fn a_name_is_held_to_the_length_of_its_kind() {
    let mut a = open();
    typed(&mut a, VariableEntity::Variable, &"a".repeat(256));
    assert_eq!(check(&a, VariableEntity::Variable).0, None);
    typed(&mut a, VariableEntity::Variable, &"a".repeat(257));
    assert_eq!(
        check(&a, VariableEntity::Variable).0,
        Some(NameError::TooLong)
    );
    // A mode is a short label: 64 characters, counted as characters.
    typed(&mut a, VariableEntity::Mode, &"é".repeat(64));
    assert_eq!(check(&a, VariableEntity::Mode).0, None);
    typed(&mut a, VariableEntity::Mode, &"é".repeat(65));
    assert_eq!(check(&a, VariableEntity::Mode).0, Some(NameError::TooLong));
}

#[test]
fn a_mode_is_free_text_and_a_collection_is_a_path() {
    let mut a = open();
    typed(&mut a, VariableEntity::Mode, "High Contrast");
    assert_eq!(check(&a, VariableEntity::Mode).0, None);
    typed(&mut a, VariableEntity::Collection, "High Contrast");
    assert_eq!(
        check(&a, VariableEntity::Collection).0,
        Some(NameError::NotPath)
    );
    typed(&mut a, VariableEntity::Collection, "theme/high-contrast");
    assert_eq!(check(&a, VariableEntity::Collection).0, None);
}

#[test]
fn check_name_in_collection_flags_duplicate() {
    let mut a = open();
    a.dispatch(op(ManageOp::Choose { id: "t".into() }));
    typed(&mut a, VariableEntity::Variable, "color/bg");
    assert_eq!(
        check(&a, VariableEntity::Variable).0,
        Some(NameError::Duplicate)
    );
    // The same name in another collection is free; the draft follows the collection that is shown.
    a.dispatch(op(ManageOp::Choose { id: "u".into() }));
    assert_eq!(check(&a, VariableEntity::Variable).0, None);
    typed(&mut a, VariableEntity::Variable, "color/accent");
    assert_eq!(
        check(&a, VariableEntity::Variable).0,
        Some(NameError::Duplicate)
    );
    a.dispatch(op(ManageOp::Choose { id: "t".into() }));
    assert_eq!(check(&a, VariableEntity::Variable).0, None);
}

#[test]
fn a_collection_or_mode_with_a_name_in_use_is_a_warning_not_an_error() {
    let mut a = open();
    typed(&mut a, VariableEntity::Collection, "Appearance");
    // Not a path (a capital); the warning comes with a name the core takes.
    typed(&mut a, VariableEntity::Collection, "appearance");
    let (error, warning, _) = check(&a, VariableEntity::Collection);
    assert_eq!((error, warning), (None, Some(NameWarning::Same)));
    a.dispatch(op(ManageOp::Choose { id: "t".into() }));
    typed(&mut a, VariableEntity::Mode, " LIGHT ");
    let (error, warning, name) = check(&a, VariableEntity::Mode);
    assert_eq!(
        (error, warning, name.as_str()),
        (None, Some(NameWarning::Same), "LIGHT")
    );
    a.dispatch(op(ManageOp::Choose { id: "u".into() }));
    assert_eq!(
        check(&a, VariableEntity::Mode).1,
        None,
        "`light` is a mode of the other collection"
    );
}

#[test]
fn a_draft_is_emptied_once_the_thing_was_made_and_kept_while_it_is_refused() {
    let mut a = open();
    typed(&mut a, VariableEntity::Collection, "theme/new");
    a.dispatch(Action::CreateCollection {
        name: "theme/new".into(),
    });
    let id = made(&a.take_events())[0].1.clone();
    assert_eq!(a.state().manage.drafts.collection.text, "");
    assert_eq!(
        a.state().manage.chosen.as_deref(),
        Some(id.as_str()),
        "the new collection is shown"
    );
    typed(&mut a, VariableEntity::Variable, "color/x");
    a.dispatch(Action::AddVariable {
        collection: id.clone(),
        name: "color/x".into(),
        kind: VarType::Color,
    });
    assert_eq!(a.state().manage.drafts.variable.text, "");
    typed(&mut a, VariableEntity::Mode, "x");
    a.dispatch(Action::AddMode {
        collection: "bogus".into(),
        name: "x".into(),
        copy_from: None,
    });
    assert!(!refused(&mut a).is_empty());
    assert_eq!(
        a.state().manage.drafts.mode.text,
        "x",
        "a refusal keeps the text for a correction"
    );
}

#[test]
fn choose_shows_a_collection_and_ends_a_question_and_a_field() {
    let mut a = open();
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("bg"),
    }));
    assert!(a.state().manage.removal.is_some());
    a.dispatch(op(ManageOp::Choose { id: "u".into() }));
    let m = &a.state().manage;
    assert_eq!(m.chosen.as_deref(), Some("u"));
    assert!(m.removal.is_none() && m.rename.is_none());
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("accent"),
    }));
    assert!(a.state().manage.rename.is_some());
    a.dispatch(op(ManageOp::Choose { id: "t".into() }));
    assert!(a.state().manage.rename.is_none());
    a.dispatch(op(ManageOp::Choose { id: "nope".into() }));
    assert_eq!(refused(&mut a), ["E_COLLECTION_NOT_FOUND"]);
    assert_eq!(a.state().manage.chosen.as_deref(), Some("t"));
}
