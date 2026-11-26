//! The question before a removal in the Variables manage view, as app state (audit finding T5, issue 322): what a removal
//! detaches is decided in the app layer from the file, and a confirmation goes ahead with it as one undo step.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, Event, ManageOp, VarType};

mod common;
use common::*;

#[test]
fn a_change_of_the_manage_state_is_an_event_so_the_front_end_reads_it_and_nothing_else_is() {
    let mut a = open();
    a.take_events();
    a.dispatch(op(ManageOp::Choose { id: "u".into() }));
    assert!(a.take_events().contains(&Event::ManageChanged {}));
    a.dispatch(op(ManageOp::Choose { id: "u".into() }));
    assert!(!a.take_events().contains(&Event::ManageChanged {}));
    a.dispatch(Action::Select {
        ids: vec!["card".into()],
    });
    assert!(!a.take_events().contains(&Event::ManageChanged {}));
    // What follows the file is a change too: a question whose numbers moved.
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("bg"),
    }));
    a.take_events();
    a.dispatch(Action::Delete {
        ids: vec!["card".into()],
    });
    assert!(a.take_events().contains(&Event::ManageChanged {}));
}

#[test]
fn removal_plan_for_used_variable_is_detach() {
    let mut a = open();
    // `bg` is used by the fill of `card` and by the alias `accent`; `round` by the radius of `card`; `accent` by `dot`.
    for (id, uses) in [("bg", 2), ("round", 1), ("accent", 1)] {
        a.dispatch(op(ManageOp::AskRemove {
            target: variable_target(id),
        }));
        let r = a.state().manage.removal.clone().unwrap();
        assert_eq!((r.detach, r.uses, r.variables), (true, uses, 0), "{id}");
    }
    a.dispatch(Action::AddVariable {
        collection: "t".into(),
        name: "color/ink".into(),
        kind: VarType::Color,
    });
    let ink = made(&a.take_events())[0].1.clone();
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target(&ink),
    }));
    let r = a.state().manage.removal.clone().unwrap();
    assert_eq!(
        (r.detach, r.uses, r.name.as_str()),
        (false, 0, "color/ink"),
        "a variable nothing uses is removed plainly, so a use that appears is refused and not detached unasked"
    );
}

#[test]
fn the_plan_of_a_collection_counts_its_variables_and_every_use_whatever_the_search_shows() {
    let mut a = open();
    a.dispatch(Action::SetVariablesQuery {
        query: "radius".into(),
    });
    a.dispatch(op(ManageOp::AskRemove {
        target: collection_target("t"),
    }));
    let r = a.state().manage.removal.clone().unwrap();
    // 2 variables; uses: `bg` twice (a node and an alias), `round` once.
    assert_eq!((r.detach, r.variables, r.uses), (true, 2, 3));
    a.dispatch(Action::CreateCollection {
        name: "empty/set".into(),
    });
    let id = made(&a.take_events())[0].1.clone();
    a.dispatch(op(ManageOp::AskRemove {
        target: collection_target(&id),
    }));
    let r = a.state().manage.removal.clone().unwrap();
    assert_eq!((r.detach, r.variables, r.uses), (false, 0, 0));
}

#[test]
fn a_mode_is_removed_plainly() {
    let mut a = open();
    a.dispatch(op(ManageOp::AskRemove {
        target: mode_target("t", "dark"),
    }));
    let r = a.state().manage.removal.clone().unwrap();
    assert_eq!((r.detach, r.uses, r.name.as_str()), (false, 0, "dark"));
}

#[test]
fn confirm_remove_detaches_one_undo_step_and_closes_the_question() {
    let mut a = open();
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("bg"),
    }));
    let steps = a.state().history.undo_steps;
    let fx = a.dispatch(op(ManageOp::ConfirmRemove {}));
    assert!(refused(&mut a).is_empty());
    assert!(variable(&a, "bg").is_none());
    assert_eq!(a.state().history.undo_steps, steps + 1);
    assert!(!sent(&fx).is_empty());
    assert!(a.state().manage.removal.is_none());
    // The node field kept the colour it painted, the alias kept its value: detached, not left dangling.
    assert_eq!(
        node(&a, "card").fill.lit().map(String::as_str),
        Some("#ffffff")
    );
    a.dispatch(Action::Undo {});
    assert!(variable(&a, "bg").is_some());
}

#[test]
fn confirm_remove_for_a_plain_collection_mode_or_unused_variable() {
    let mut a = open();
    a.dispatch(op(ManageOp::AskRemove {
        target: mode_target("t", "dark"),
    }));
    a.dispatch(op(ManageOp::ConfirmRemove {}));
    assert!(refused(&mut a).is_empty());
    assert_eq!(a.vars().collection("t").unwrap().modes.len(), 1);
    a.dispatch(Action::CreateCollection {
        name: "empty/set".into(),
    });
    let id = made(&a.take_events())[0].1.clone();
    a.dispatch(op(ManageOp::AskRemove {
        target: collection_target(&id),
    }));
    a.dispatch(op(ManageOp::ConfirmRemove {}));
    assert!(a.vars().collection(&id).is_none());
}

#[test]
fn confirm_without_a_question_does_nothing() {
    let mut a = open();
    let steps = a.state().history.undo_steps;
    let fx = a.dispatch(op(ManageOp::ConfirmRemove {}));
    assert!(fx.is_empty() && refused(&mut a).is_empty());
    assert_eq!(a.state().history.undo_steps, steps);
    // And a confirmation is answered once.
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("round"),
    }));
    a.dispatch(op(ManageOp::ConfirmRemove {}));
    a.dispatch(op(ManageOp::ConfirmRemove {}));
    assert_eq!(a.state().history.undo_steps, steps + 1);
}

#[test]
fn the_plan_follows_the_file_while_the_question_is_open_and_ends_with_its_target() {
    let mut a = open();
    a.dispatch(Action::AddVariable {
        collection: "t".into(),
        name: "color/ink".into(),
        kind: VarType::Color,
    });
    let ink = made(&a.take_events())[0].1.clone();
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target(&ink),
    }));
    assert!(!a.state().manage.removal.as_ref().unwrap().detach);
    // A node field gets bound to it: the question now says what a confirmation does.
    a.dispatch(Action::Select {
        ids: vec!["plain".into()],
    });
    a.dispatch(Action::BindProperty {
        id: "plain".into(),
        field: meldframe_app::BindField::Fill,
        variable: Some(ink.clone()),
    });
    let r = a.state().manage.removal.clone().unwrap();
    assert_eq!((r.detach, r.uses), (true, 1));
    // It goes (here by the same person from elsewhere, the rule is the same for another actor's change): the question ends, quietly.
    a.take_events();
    a.dispatch(Action::RemoveVariable {
        id: ink,
        detach: true,
    });
    assert!(a.state().manage.removal.is_none());
    assert!(refused(&mut a).is_empty());
}

#[test]
fn asking_to_remove_what_is_not_there_is_refused_by_kind() {
    let mut a = open();
    for (target, code) in [
        (variable_target("nope"), "E_VARIABLE_NOT_FOUND"),
        (collection_target("nope"), "E_COLLECTION_NOT_FOUND"),
        (mode_target("t", "nope"), "E_MODE_NOT_FOUND"),
        (mode_target("nope", "light"), "E_COLLECTION_NOT_FOUND"),
    ] {
        a.dispatch(op(ManageOp::AskRemove {
            target: target.clone(),
        }));
        assert_eq!(refused(&mut a), [code], "{target:?}");
        a.dispatch(op(ManageOp::StartRename { target }));
        assert_eq!(refused(&mut a), [code]);
    }
    assert!(a.state().manage.removal.is_none() && a.state().manage.rename.is_none());
}

#[test]
fn a_question_and_a_field_exclude_each_other() {
    let mut a = open();
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("bg"),
    }));
    assert!(a.state().manage.rename.is_none() && a.state().manage.removal.is_some());
    a.dispatch(op(ManageOp::StartRename {
        target: variable_target("bg"),
    }));
    assert!(a.state().manage.rename.is_some() && a.state().manage.removal.is_none());
    a.dispatch(op(ManageOp::CancelRename {}));
    assert!(a.state().manage.rename.is_none());
    a.dispatch(op(ManageOp::AskRemove {
        target: variable_target("bg"),
    }));
    a.dispatch(op(ManageOp::CancelRemove {}));
    assert!(a.state().manage.removal.is_none());
}
