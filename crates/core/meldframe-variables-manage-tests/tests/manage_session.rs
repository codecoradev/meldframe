//! What the Variables manage view keeps between uses (audit finding T5, issue 322): closing the view forgets the question,
//! the rename field, the three forms and the collection shown; a new collection ends a question and a field; a text that is
//! pasted is held to a size; the collection shown is said once; and a refused save says why in its own code.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, ManageOp, VariableEntity};
mod common;
use common::*;

fn chosen(a: &meldframe_app::App) -> Option<&str> {
    a.state().manage.shown.as_deref()
}

#[test]
fn closing_the_view_forgets_the_question_the_field_the_forms_and_the_collection() {
    let mut a = open();
    a.dispatch(op(ManageOp::Choose { id: "u".into() }));
    for form in [
        VariableEntity::Collection,
        VariableEntity::Mode,
        VariableEntity::Variable,
    ] {
        typed(&mut a, form, "draft/name");
    }
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("accent"),
    }));
    assert!(a.state().manage.removal.is_some());
    a.dispatch(op(ManageOp::Close {}));
    let m = a.state().manage.clone();
    assert!(m.removal.is_none() && m.rename.is_none());
    assert_eq!(
        m.drafts,
        open().state().manage.drafts,
        "the three forms start again"
    );
    assert!(m.chosen.is_none());
    assert_eq!(chosen(&a), Some("t"), "the first collection is shown again");

    a.dispatch(op(ManageOp::StartRename {
        target: collection_target("t"),
    }));
    assert!(a.state().manage.rename.is_some());
    a.dispatch(op(ManageOp::Close {}));
    assert!(a.state().manage.rename.is_none());
    // Closed twice, or with nothing open: nothing to say and nothing refused.
    a.take_events();
    a.dispatch(op(ManageOp::Close {}));
    assert!(a.take_events().is_empty());
}

#[test]
fn a_field_left_open_does_not_refuse_anything_once_the_view_is_closed() {
    let mut a = open();
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    a.dispatch(op(ManageOp::Close {}));
    a.take_events();
    // Another actor removes the variable while the view is closed: no field of ours is left to be refused.
    a.dispatch(Action::RemoveVariable {
        id: "bg".into(),
        detach: true,
    });
    assert!(refused(&mut a).is_empty());
    assert!(a.state().manage.rename.is_none());
}

#[test]
fn a_question_asked_before_the_close_is_not_asked_again_on_reopening() {
    let mut a = open();
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("bg"),
    }));
    a.dispatch(op(ManageOp::Close {}));
    let fx = a.dispatch(op(ManageOp::ConfirmRemove {}));
    assert!(
        sent(&fx).is_empty(),
        "a Detach nobody asked for after the close"
    );
    assert!(variable(&a, "bg").is_some());
}

#[test]
fn a_new_collection_ends_the_question_and_the_field() {
    let mut a = open();
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("bg"),
    }));
    typed(&mut a, VariableEntity::Collection, "fresh");
    a.dispatch(Action::CreateCollection {
        name: "fresh".into(),
    });
    let m = &a.state().manage;
    assert!(
        m.removal.is_none(),
        "the question was about the collection before"
    );
    assert!(m.drafts.collection.text.is_empty());

    let mut a = open();
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    typed(&mut a, VariableEntity::Collection, "fresh");
    a.dispatch(Action::CreateCollection {
        name: "fresh".into(),
    });
    assert!(a.state().manage.rename.is_none());
    assert_ne!(chosen(&a), Some("t"), "the new collection is the one shown");
}

#[test]
fn a_pasted_text_is_kept_to_a_size_and_checked_as_too_long() {
    let mut a = open();
    let huge = "a".repeat(5000);
    let max = a.state().manage.limits.name_max;
    typed(&mut a, VariableEntity::Variable, &huge);
    let d = &a.state().manage.drafts.variable;
    assert_eq!(d.text.chars().count(), 4 * max);
    assert_eq!(d.check.error, Some(meldframe_app::NameError::TooLong));

    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    a.dispatch(op(ManageOp::SetRenameText { text: huge }));
    let r = a.state().manage.rename.clone().unwrap();
    assert_eq!(r.text.chars().count(), 4 * max);
    assert_eq!(r.check.error, Some(meldframe_app::NameError::TooLong));
    // A text under the cap is kept whole, multi-byte characters counted as characters.
    typed(&mut a, VariableEntity::Mode, &"é".repeat(100));
    assert_eq!(a.state().manage.drafts.mode.text.chars().count(), 100);
}

#[test]
fn the_collection_shown_is_one_id_in_the_state() {
    let mut a = open();
    assert_eq!(chosen(&a), Some("t"), "no choice: the first");
    a.dispatch(op(ManageOp::Choose { id: "u".into() }));
    assert_eq!(chosen(&a), Some("u"));
    // Another actor removes the collection shown: the first is shown again.
    a.dispatch(Action::RemoveCollection {
        id: "u".into(),
        detach: true,
    });
    assert_eq!(chosen(&a), Some("t"));
    assert!(a.state().manage.chosen.is_none());
}

#[test]
fn a_refused_save_says_why_in_a_code_of_its_own() {
    let mut a = open();
    for (text, code) in [
        ("", "name_empty"),
        ("a\u{200b}b", "name_hidden"),
        ("Not A Path", "E_INVALID_NAME"),
        ("color/bg-", "E_INVALID_NAME"),
        ("radius/card", "E_DUPLICATE_NAME"),
        (&"a".repeat(300), "E_TOO_LONG"),
    ] {
        a.dispatch(op(ManageOp::StartRename {
            target: variable_target("bg"),
        }));
        a.dispatch(op(ManageOp::SetRenameText { text: text.into() }));
        a.take_events();
        a.dispatch(op(ManageOp::SubmitRename {}));
        assert_eq!(refused(&mut a), [code], "{text:?}");
        assert!(
            a.state().manage.rename.is_some(),
            "the field stays, with its text"
        );
    }
}
