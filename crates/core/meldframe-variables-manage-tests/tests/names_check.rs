//! The checks of a typed name in the Variables manage view (audit finding T5, issue 322), asked of the pure part of the app
//! layer (`meldframe_app_parts::manage_names`) with no app: the name rule of the core, what hides a meaning, the length of
//! each kind, the names a new thing is compared with, and what a rename is held against.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app_model::{MODE_NAME_MAX, ManageLimits, NameError, NameWarning, VariableEntity};
use meldframe_app_parts::manage_names::{check_name, check_rename};

fn error(entity: VariableEntity, text: &str) -> Option<NameError> {
    check_name(entity, text, &[]).error
}

#[test]
fn a_variable_or_collection_is_a_lowercase_path_and_a_mode_is_free_text() {
    for entity in [VariableEntity::Variable, VariableEntity::Collection] {
        assert_eq!(error(entity, "color/brand-500"), None);
        assert_eq!(error(entity, "Color/Brand"), Some(NameError::NotPath));
        assert_eq!(error(entity, "a--b"), Some(NameError::NotPath));
        assert_eq!(error(entity, "a//b"), Some(NameError::NotPath));
        assert_eq!(error(entity, "  "), Some(NameError::Empty));
    }
    assert_eq!(error(VariableEntity::Mode, "High Contrast"), None);
}

#[test]
fn what_hides_a_meaning_is_refused_and_an_emoji_with_its_selector_is_not() {
    // A sun with variation selector 16, a family, a keycap: names a person means.
    for text in [
        "\u{2600}\u{fe0f}",
        "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}",
        "1\u{fe0f}\u{20e3}",
    ] {
        assert_eq!(error(VariableEntity::Mode, text), None, "{text:?}");
    }
    for text in [
        "a\u{feff}b",
        "a\u{202e}b",
        "a\u{200b}b",
        "a\u{a0}b",
        "a\tb",
        "a\u{fe0f}b",
        "a\u{e0100}b",
    ] {
        assert_eq!(
            error(VariableEntity::Mode, text),
            Some(NameError::Hidden),
            "{text:?}"
        );
    }
}

#[test]
fn a_name_is_held_to_the_length_of_its_kind_in_characters() {
    let limits = ManageLimits::default();
    let long = |n: usize| "é".repeat(n);
    assert_eq!(error(VariableEntity::Mode, &long(MODE_NAME_MAX)), None);
    assert_eq!(
        error(VariableEntity::Mode, &long(MODE_NAME_MAX + 1)),
        Some(NameError::TooLong)
    );
    let path = |n: usize| "a".repeat(n);
    assert_eq!(
        error(VariableEntity::Variable, &path(limits.name_max)),
        None
    );
    assert_eq!(
        error(VariableEntity::Variable, &path(limits.name_max + 1)),
        Some(NameError::TooLong)
    );
}

#[test]
fn a_variable_that_exists_is_an_error_and_a_collection_or_mode_is_a_warning() {
    let taken = ["color/bg"];
    let c = check_name(VariableEntity::Variable, " color/bg ", &taken);
    assert_eq!(
        (c.error, c.name.as_str()),
        (Some(NameError::Duplicate), "color/bg")
    );
    let c = check_name(VariableEntity::Collection, "color/bg", &taken);
    assert_eq!((c.error, c.warning), (None, Some(NameWarning::Same)));
    let c = check_name(VariableEntity::Mode, "Dark", &["dark"]);
    assert_eq!((c.error, c.warning), (None, Some(NameWarning::Same)));
}

#[test]
fn a_rename_to_the_name_it_has_or_opened_with_is_no_change() {
    let others = vec!["color/ink".to_string()];
    for text in ["color/bg", "  color/bg ", "color/old"] {
        let (check, unchanged) = check_rename(
            VariableEntity::Variable,
            text,
            "color/bg",
            "color/old",
            &others,
        );
        assert!(unchanged && check.error.is_none(), "{text:?}");
    }
    let (check, unchanged) = check_rename(
        VariableEntity::Variable,
        "color/ink",
        "color/bg",
        "color/bg",
        &others,
    );
    assert!(!unchanged);
    assert_eq!(check.error, Some(NameError::Duplicate));
}
