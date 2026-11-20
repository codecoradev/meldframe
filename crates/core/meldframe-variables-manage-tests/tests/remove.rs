//! The Variables manage actions: remove a variable or a collection, with or without letting go of what uses it; the detach at the gate
//! and against another actor's changes.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::Action;
use meldframe_host::Op;
use meldframe_module::RiskTier;
use meldframe_protocol::{Bind, Command, NodePatch, Value, VarType};
use serde_json::json;

mod common;
use common::*;

fn remove_variable(id: &str, detach: bool) -> Action {
    Action::RemoveVariable {
        id: id.into(),
        detach,
    }
}

fn remove_collection(id: &str, detach: bool) -> Action {
    Action::RemoveCollection {
        id: id.into(),
        detach,
    }
}

#[test]
fn a_variable_nothing_uses_is_removed_with_either_flag() {
    for detach in [false, true] {
        let mut a = open();
        a.dispatch(Action::AddVariable {
            collection: "t".into(),
            name: "color/ink".into(),
            kind: VarType::Color,
        });
        let id = made(&a.take_events())[0].1.clone();
        let steps = a.state().history.undo_steps;
        let fx = a.dispatch(remove_variable(&id, detach));
        assert!(refused(&mut a).is_empty(), "detach {detach}");
        assert!(variable(&a, &id).is_none());
        assert_eq!(sent(&fx).len(), 1, "a lone removeVariable");
        assert_eq!(a.state().history.undo_steps, steps + 1);
    }
}

#[test]
fn a_variable_in_use_is_refused_without_detach_and_nothing_changes() {
    let mut a = open();
    let before = snapshot(&a);
    for id in ["bg", "round", "accent"] {
        let fx = a.dispatch(remove_variable(id, false));
        assert_eq!(refused(&mut a), ["E_VARIABLE_IN_USE"], "{id}");
        assert!(sent(&fx).is_empty());
    }
    assert_eq!(snapshot(&a), before);
    assert!(a.state().pending.is_empty());
}

#[test]
fn unknown_ids_are_refused_with_the_core_codes() {
    let mut a = open();
    for detach in [false, true] {
        a.dispatch(remove_variable("nope", detach));
        a.dispatch(remove_collection("nope", detach));
        assert_eq!(
            refused(&mut a),
            ["E_VARIABLE_NOT_FOUND", "E_COLLECTION_NOT_FOUND"]
        );
    }
    assert!(a.state().pending.is_empty());
}

#[test]
fn detach_gives_each_field_and_alias_the_value_it_had_in_the_default_mode_and_removes_as_one_step()
{
    let mut a = open();
    // The canvas shows `dark`; the literal is still the one of the default mode (docs/tokens-and-variables.md 5.2).
    dark(&mut a);
    let steps = a.state().history.undo_steps;
    let fx = a.dispatch(remove_variable("bg", true));
    assert!(refused(&mut a).is_empty());
    assert!(variable(&a, "bg").is_none());
    assert_eq!(node(&a, "card").fill, Bind::from("#ffffff"));
    assert_eq!(
        node(&a, "card").radius,
        Bind::var("round"),
        "only the fields of bg are let go"
    );
    assert_eq!(
        lit(&variable(&a, "accent").unwrap().values["base"]),
        Value::Color("#ffffff".into())
    );
    assert_eq!(a.state().history.undo_steps, steps + 1, "one undo step");
    let kinds: Vec<_> = sent(&fx).into_iter().map(|o| o.command).collect();
    assert!(
        matches!(kinds.last(), Some(Command::RemoveVariable { id }) if id == "bg"),
        "{kinds:?}"
    );
    assert_eq!(
        kinds.len(),
        3,
        "the node, the alias, the removal: {kinds:?}"
    );
    assert!(a.state().pending.len() == 3);
    // The canvas paints what it painted before for the default mode.
    assert_eq!(
        a.scene().iter().find(|b| b.id == "card").unwrap().fill,
        Some(0xffffff)
    );
}

#[test]
fn detach_of_a_number_takes_the_default_mode_too() {
    let mut a = open();
    dark(&mut a);
    a.dispatch(remove_variable("round", true));
    assert!(refused(&mut a).is_empty());
    assert_eq!(node(&a, "card").radius, Bind::Lit(4.0));
    assert_eq!(node(&a, "card").fill, Bind::var("bg"));
}

#[test]
fn an_alias_keeps_the_value_it_had_in_its_own_mode() {
    let mut a = open();
    a.dispatch(Action::EditVariables {
        commands: vec![cmd(json!({"type": "addVariable", "variable": {
            "id": "tint", "collection": "t", "name": "color/tint", "type": "color",
            "values": {"light": {"var": "bg"}, "dark": {"var": "bg"}}}}))],
    });
    a.dispatch(remove_variable("bg", true));
    assert!(refused(&mut a).is_empty());
    let tint = variable(&a, "tint").unwrap();
    assert_eq!(tint.values["light"], color("#ffffff"));
    assert_eq!(tint.values["dark"], color("#000000"));
}

#[test]
fn undo_of_a_detach_restores_the_variable_the_binding_and_the_alias_in_one_step() {
    let mut a = open();
    let before = snapshot(&a);
    a.dispatch(remove_variable("bg", true));
    assert_ne!(snapshot(&a), before);
    assert_eq!(a.state().history.undo_steps, 1);
    a.dispatch(Action::Undo {});
    assert!(refused(&mut a).is_empty());
    assert_eq!(snapshot(&a), before);
    let h = &a.state().history;
    assert_eq!(
        (h.undo_steps, h.redo_steps, h.can_undo, h.can_redo),
        (0, 1, false, true),
        "the whole detach was one step"
    );
    a.dispatch(Action::Redo {});
    assert!(variable(&a, "bg").is_none());
    assert_eq!(node(&a, "card").fill, Bind::from("#ffffff"));
}

#[test]
fn a_collection_with_variables_is_refused_without_detach() {
    let mut a = open();
    let before = snapshot(&a);
    let fx = a.dispatch(remove_collection("t", false));
    assert_eq!(refused(&mut a), ["E_COLLECTION_NOT_EMPTY"]);
    assert!(sent(&fx).is_empty());
    assert_eq!(snapshot(&a), before);
}

#[test]
fn an_empty_collection_is_removed_with_either_flag() {
    for detach in [false, true] {
        let mut a = open();
        a.dispatch(Action::CreateCollection { name: "tmp".into() });
        let id = made(&a.take_events())[0].1.clone();
        a.dispatch(remove_collection(&id, detach));
        assert!(refused(&mut a).is_empty());
        assert!(a.vars().collection(&id).is_none());
    }
}

#[test]
fn detach_of_a_collection_lets_go_of_every_user_and_removes_its_variables_in_one_step() {
    let mut a = open();
    // `tint` aliases `bg` inside the collection: it goes first, then `bg`.
    a.dispatch(Action::EditVariables {
        commands: vec![cmd(json!({"type": "addVariable", "variable": {
            "id": "tint", "collection": "t", "name": "color/tint", "type": "color",
            "values": {"light": {"var": "bg"}, "dark": {"var": "bg"}}}}))],
    });
    a.take_events();
    let before = snapshot(&a);
    let steps = a.state().history.undo_steps;
    dark(&mut a);
    a.dispatch(remove_collection("t", true));
    assert!(refused(&mut a).is_empty());
    assert!(a.vars().collection("t").is_none());
    assert!(
        ["bg", "round", "tint"]
            .iter()
            .all(|v| variable(&a, v).is_none())
    );
    assert_eq!(node(&a, "card").fill, Bind::from("#ffffff"));
    assert_eq!(node(&a, "card").radius, Bind::Lit(4.0));
    assert_eq!(
        lit(&variable(&a, "accent").unwrap().values["base"]),
        Value::Color("#ffffff".into())
    );
    assert_eq!(a.state().history.undo_steps, steps + 1, "one undo step");
    assert!(
        a.state().modes.is_empty(),
        "the pick of a removed collection is gone"
    );
    a.dispatch(Action::Undo {});
    assert!(refused(&mut a).is_empty());
    assert_eq!(snapshot(&a), before);
}

#[test]
fn a_group_over_the_queue_is_refused_whole() {
    let mut a = open();
    a.set_max_pending(2);
    let before = snapshot(&a);
    let steps = a.state().history.undo_steps;
    let fx = a.dispatch(remove_variable("bg", true));
    assert_eq!(refused(&mut a), ["outbox_full"]);
    assert!(sent(&fx).is_empty());
    assert_eq!(snapshot(&a), before);
    assert_eq!(
        (a.state().pending.len(), a.state().history.undo_steps),
        (0, steps)
    );
    // The refusal is about the size: a lone removal still fits.
    a.dispatch(Action::AddVariable {
        collection: "t".into(),
        name: "color/ink".into(),
        kind: VarType::Color,
    });
    let id = made(&a.take_events())[0].1.clone();
    a.dispatch(remove_variable(&id, true));
    assert!(refused(&mut a).is_empty());
}

fn recolour(id: &str, css: &str) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            fill: Some(Bind::from(css)),
            ..Default::default()
        },
    }
}

#[test]
fn a_detach_is_change_tier_and_never_asks_for_approval() {
    let mut a = open();
    let fx = a.dispatch(Action::RemoveCollection {
        id: "t".into(),
        detach: true,
    });
    let ops: Vec<Op> = sent(&fx)
        .into_iter()
        .map(|o| Op::new(o.key, o.command).with_deps(o.deps))
        .collect();
    assert!(ops.len() > 3, "a group of several commands: {}", ops.len());
    assert!(
        ops.iter()
            .all(|o| !matches!(o.command, Command::RemoveNode { .. })),
        "a removal of variables removes no node"
    );
    // Whatever the facts about the tree, no op of the group is a Delete: the gate lets it through.
    assert!(
        Op::batch_tiers(&ops, &())
            .iter()
            .all(|t| *t == RiskTier::Change)
    );
}

#[test]
fn undo_of_a_detach_keeps_what_another_actor_wrote_to_the_field_after_it() {
    let mut a = open();
    let fx = a.dispatch(Action::RemoveVariable {
        id: "bg".into(),
        detach: true,
    });
    let seq = confirm(&mut a, "user:a", sent(&fx), 0);
    assert!(a.state().pending.is_empty());
    // B paints the card red after the detach.
    remote(&mut a, seq + 1, recolour("card", "#ff0000"));
    a.take_events();
    a.dispatch(Action::Undo {});
    assert!(refused(&mut a).is_empty());
    // The variable and the alias are back; the colour B wrote stays.
    assert!(variable(&a, "bg").is_some());
    assert_eq!(node(&a, "card").fill, Bind::from("#ff0000"));
    assert_eq!(
        variable(&a, "accent").unwrap().values["base"],
        Bind::var("bg")
    );
}

#[test]
fn a_remote_bind_that_comes_first_keeps_the_variable_and_the_rest_of_the_group() {
    let mut a = open();
    let fx = a.dispatch(Action::RemoveVariable {
        id: "bg".into(),
        detach: true,
    });
    let mine = sent(&fx);
    assert_eq!(mine.len(), 3);
    a.take_events();
    // B binds `plain` to `bg` and the server orders it before my group: the removal can no longer apply.
    remote(
        &mut a,
        1,
        Command::UpdateNode {
            id: "plain".into(),
            patch: NodePatch {
                fill: Some(Bind::var("bg")),
                ..Default::default()
            },
        },
    );
    assert!(
        variable(&a, "bg").is_some(),
        "the variable still has a user"
    );
    assert_eq!(node(&a, "plain").fill, Bind::var("bg"));
    assert_eq!(
        node(&a, "card").fill,
        Bind::from("#ffffff"),
        "what the group let go stays let go"
    );
    assert_eq!(
        a.state().pending.len(),
        2,
        "the removal fell, the two before it stay"
    );
    // No field is bound to a variable that is gone.
    for n in a.nodes() {
        for b in meldframe_variables::node_bindings(n) {
            assert!(
                variable(&a, b.var).is_some(),
                "{} is bound to a missing {}",
                n.id,
                b.var
            );
        }
    }
    // The server takes the two that are left, and the view stays as it is.
    let view = format!("{:?}{:?}", a.nodes(), a.vars());
    let left: Vec<_> = mine
        .into_iter()
        .filter(|o| a.state().pending.contains(&o.key))
        .collect();
    assert_eq!(left.len(), 2);
    confirm(&mut a, "user:a", left, 1);
    assert!(a.state().pending.is_empty());
    assert_eq!(format!("{:?}{:?}", a.nodes(), a.vars()), view);
}
