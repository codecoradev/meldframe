//! The rename flow of the Variables manage view, as app state (audit finding T5, issue 322): the field a name is edited in,
//! its checks, the one op a save sends, and what happens to a field when another actor changes or removes its target.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, Event, ManageOp, NameError, NameWarning, VariableEntity};
use meldframe_protocol::Command;
use serde_json::json;

mod common;
use common::*;

#[test]
fn rename_to_own_name_is_unchanged() {
    let mut a = open();
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    let r = a.state().manage.rename.clone().unwrap();
    assert_eq!(
        (r.text.as_str(), r.opened.as_str(), r.current.as_str()),
        ("color/bg", "color/bg", "color/bg")
    );
    assert!(r.unchanged && r.check.error.is_none());
    let steps = a.state().history.undo_steps;
    a.take_events();
    let fx = a.dispatch(op(ManageOp::SubmitRename {}));
    assert!(sent(&fx).is_empty());
    assert_eq!(
        a.state().history.undo_steps,
        steps,
        "the core would take it and record a useless step"
    );
    assert!(
        a.state().manage.rename.is_none(),
        "saving the name it has is a quiet cancel"
    );
    let events = a.take_events();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Renamed { .. } | Event::Refused { .. }))
    );
}

#[test]
fn a_rename_is_one_op_one_undo_step_and_says_what_was_renamed() {
    let mut a = open();
    for (target, entity, text, from, to) in [
        (
            variable_target("bg"),
            VariableEntity::Variable,
            " color/surface ",
            "color/bg",
            "color/surface",
        ),
        (
            collection_target("t"),
            VariableEntity::Collection,
            "theme/main",
            "appearance",
            "theme/main",
        ),
        (
            mode_target("t", "dark"),
            VariableEntity::Mode,
            "Night mode",
            "dark",
            "Night mode",
        ),
    ] {
        a.dispatch(op(ManageOp::StartRename { target }));
        a.dispatch(op(ManageOp::SetRenameText { text: text.into() }));
        let r = a.state().manage.rename.clone().unwrap();
        assert!(!r.unchanged && r.check.error.is_none());
        let steps = a.state().history.undo_steps;
        a.take_events();
        let fx = a.dispatch(op(ManageOp::SubmitRename {}));
        assert_eq!(sent(&fx).len(), 1, "{to}");
        assert_eq!(a.state().history.undo_steps, steps + 1);
        assert!(a.state().manage.rename.is_none());
        let events = a.take_events();
        assert!(events.contains(&Event::Renamed {
            entity,
            from: from.into(),
            to: to.into()
        }));
        assert!(!events.iter().any(|e| matches!(e, Event::Refused { .. })));
    }
    assert_eq!(variable(&a, "bg").unwrap().name, "color/surface");
    assert_eq!(a.vars().collection("t").unwrap().name, "theme/main");
    assert_eq!(
        a.vars().collection("t").unwrap().modes[1].name,
        "Night mode"
    );
    a.dispatch(Action::Undo {});
    assert_eq!(a.vars().collection("t").unwrap().modes[1].name, "dark");
}

#[test]
fn a_name_that_cannot_be_sent_keeps_the_field_with_its_text() {
    let mut a = open();
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    for (text, want) in [
        ("Color/Bg", NameError::NotPath),
        ("radius/card", NameError::Duplicate),
        ("", NameError::Empty),
    ] {
        a.dispatch(op(ManageOp::SetRenameText { text: text.into() }));
        let r = a.state().manage.rename.clone().unwrap();
        assert_eq!(r.check.error, Some(want), "{text:?}");
        assert_eq!(r.text, text);
        let steps = a.state().history.undo_steps;
        a.take_events();
        let fx = a.dispatch(op(ManageOp::SubmitRename {}));
        assert!(sent(&fx).is_empty());
        assert_eq!(a.state().history.undo_steps, steps);
        assert!(
            a.state().manage.rename.is_some(),
            "{text:?}: the field stays for a correction"
        );
        assert_eq!(refused(&mut a).len(), 1);
    }
}

#[test]
fn the_own_name_is_not_a_duplicate_and_a_collection_name_in_use_is_only_a_warning() {
    let mut a = open();
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    a.dispatch(op(ManageOp::SetRenameText {
        text: "color/bg ".into(),
    }));
    assert!(a.state().manage.rename.as_ref().unwrap().unchanged);
    a.dispatch(op(ManageOp::StartRename {
        target: collection_target("u"),
    }));
    a.dispatch(op(ManageOp::SetRenameText {
        text: "appearance".into(),
    }));
    let r = a.state().manage.rename.clone().unwrap();
    assert_eq!(
        (r.check.error, r.check.warning),
        (None, Some(NameWarning::Same))
    );
    a.dispatch(op(ManageOp::SubmitRename {}));
    assert_eq!(a.vars().collection("u").unwrap().name, "appearance");
}

#[test]
fn the_first_mode_is_marked_because_the_export_does_not_name_it() {
    let mut a = open();
    for (id, first) in [("light", true), ("dark", false)] {
        a.dispatch(op(ManageOp::StartRename {
            target: mode_target("t", id),
        }));
        assert_eq!(
            a.state().manage.rename.as_ref().unwrap().first,
            first,
            "{id}"
        );
    }
}

#[test]
fn a_rename_by_another_actor_under_an_open_field_is_never_undone_by_an_untouched_save() {
    let mut a = open();
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    remote(
        &mut a,
        1,
        Command::RenameVariable {
            id: "bg".into(),
            name: "color/page".into(),
        },
    );
    let r = a.state().manage.rename.clone().unwrap();
    assert_eq!(
        (r.opened.as_str(), r.current.as_str()),
        ("color/bg", "color/page")
    );
    assert!(r.unchanged, "the text is the name the field opened with");
    let steps = a.state().history.undo_steps;
    a.dispatch(op(ManageOp::SubmitRename {}));
    assert_eq!(a.state().history.undo_steps, steps);
    assert_eq!(variable(&a, "bg").unwrap().name, "color/page");
}

#[test]
fn a_field_ends_when_its_target_is_removed_and_says_what_is_gone() {
    for (target, remove, code) in [
        (
            variable_target("round"),
            Action::RemoveVariable {
                id: "round".into(),
                detach: true,
            },
            "E_VARIABLE_NOT_FOUND",
        ),
        (
            mode_target("t", "dark"),
            Action::RemoveMode {
                collection: "t".into(),
                mode: "dark".into(),
            },
            "E_MODE_NOT_FOUND",
        ),
        (
            collection_target("u"),
            Action::RemoveCollection {
                id: "u".into(),
                detach: true,
            },
            "E_COLLECTION_NOT_FOUND",
        ),
    ] {
        let mut a = open();
        a.dispatch(op(ManageOp::StartRename { target }));
        a.take_events();
        a.dispatch(remove);
        assert!(a.state().manage.rename.is_none(), "{code}");
        assert_eq!(refused(&mut a), [code]);
    }
}

#[test]
fn a_mode_whose_collection_goes_ends_the_field_as_a_missing_collection() {
    let mut a = open();
    a.dispatch(op(ManageOp::StartRename {
        target: mode_target("u", "base"),
    }));
    a.take_events();
    a.dispatch(Action::RemoveCollection {
        id: "u".into(),
        detach: true,
    });
    assert!(a.state().manage.rename.is_none());
    assert_eq!(refused(&mut a), ["E_COLLECTION_NOT_FOUND"]);
}

#[test]
fn a_text_with_no_field_open_does_nothing() {
    let mut a = open();
    let before = a.state().manage.clone();
    a.dispatch(op(ManageOp::SetRenameText { text: "x".into() }));
    a.dispatch(op(ManageOp::SubmitRename {}));
    a.dispatch(op(ManageOp::CancelRename {}));
    a.dispatch(op(ManageOp::CancelRemove {}));
    assert_eq!(a.state().manage, before);
    assert!(refused(&mut a).is_empty());
}

#[test]
fn the_manage_view_is_view_state_when_it_sends_nothing() {
    let mut a = open();
    let steps = a.state().history.undo_steps;
    for o in [
        ManageOp::Choose { id: "u".into() },
        ManageOp::SetDraft {
            form: VariableEntity::Variable,
            text: "x".into(),
        },
        ManageOp::AskRemove {
            target: variable_target("bg"),
        },
        ManageOp::CancelRemove {},
        ManageOp::StartRename {
            target: variable_target("accent"),
        },
        ManageOp::SetRenameText {
            text: "color/y".into(),
        },
        ManageOp::CancelRename {},
    ] {
        assert!(o.is_view());
        assert!(sent(&a.dispatch(op(o))).is_empty());
    }
    assert_eq!(a.state().history.undo_steps, steps);
}

#[test]
fn the_ops_have_a_stable_wire_shape() {
    let a: Action = serde_json::from_value(json!({
        "type": "manage",
        "op": {"type": "askRemove", "target": {"kind": "mode", "collection": "t", "id": "dark"}}
    }))
    .unwrap();
    assert!(matches!(
        a,
        Action::Manage {
            op: ManageOp::AskRemove { .. }
        }
    ));
    assert!(
        serde_json::from_value::<Action>(json!({
            "type": "manage",
            "op": {"type": "setDraft", "form": "mode", "text": "x", "extra": 1}
        }))
        .is_err()
    );
}
