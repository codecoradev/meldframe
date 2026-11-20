//! The Variables manage actions: chains of aliases, other modes, the limits, what the core lets through, and the check
//! of a group of commands before any is applied.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::Action;
use meldframe_app_parts::manage::{Target, removal};
use meldframe_app_rules::modes::check_group;
use meldframe_protocol::{Bind, Command, NodePatch};
use serde_json::json;

mod common;
use common::*;

fn add(a: &mut meldframe_app::App, v: serde_json::Value) {
    a.dispatch(Action::EditVariables {
        commands: vec![cmd(json!({"type": "addVariable", "variable": v}))],
    });
    assert!(refused(a).is_empty());
}

fn alias_of(id: &str, collection: &str, modes: &[&str], to: &str) -> serde_json::Value {
    let values: serde_json::Map<_, _> = modes
        .iter()
        .map(|m| (m.to_string(), json!({"var": to})))
        .collect();
    json!({"id": id, "collection": collection, "name": format!("color/{id}"), "type": "color", "values": values})
}

#[test]
fn a_chain_of_three_aliases_is_let_go_at_its_first_link_and_removed_in_order() {
    let mut a = open();
    add(&mut a, alias_of("tint", "t", &["light", "dark"], "bg"));
    add(&mut a, alias_of("tint2", "t", &["light", "dark"], "tint"));
    // Removing the root lets go of the alias that points at it; the one behind it keeps pointing at that alias.
    a.dispatch(Action::RemoveVariable {
        id: "bg".into(),
        detach: true,
    });
    assert!(refused(&mut a).is_empty());
    assert_eq!(
        variable(&a, "tint").unwrap().values["dark"],
        color("#000000")
    );
    assert_eq!(
        variable(&a, "tint2").unwrap().values["dark"],
        Bind::var("tint")
    );
    a.dispatch(Action::Undo {});
    let before = snapshot(&a);
    // The whole collection goes: the end of the chain first, the root last.
    a.dispatch(Action::RemoveCollection {
        id: "t".into(),
        detach: true,
    });
    assert!(refused(&mut a).is_empty());
    assert!(
        ["bg", "round", "tint", "tint2"]
            .iter()
            .all(|v| variable(&a, v).is_none())
    );
    assert_eq!(
        variable(&a, "accent").unwrap().values["base"],
        color("#ffffff")
    );
    a.dispatch(Action::Undo {});
    assert_eq!(snapshot(&a), before);
}

#[test]
fn an_alias_in_another_collection_keeps_the_default_mode_of_the_collection_it_points_into() {
    let mut a = open();
    a.dispatch(Action::EditVariables {
        commands: vec![cmd(
            json!({"type": "addCollection", "collection": {"id": "v", "name": "pick",
            "modes": [{"id": "a", "name": "a"}, {"id": "b", "name": "b"}]}}),
        )],
    });
    add(&mut a, alias_of("pick", "v", &["a", "b"], "bg"));
    dark(&mut a);
    a.dispatch(Action::RemoveVariable {
        id: "bg".into(),
        detach: true,
    });
    assert!(refused(&mut a).is_empty());
    // Mode `b` is not the default one, but `t` is read in its default mode, whatever the view shows.
    let pick = variable(&a, "pick").unwrap();
    assert_eq!(
        (&pick.values["a"], &pick.values["b"]),
        (&color("#ffffff"), &color("#ffffff"))
    );
}

#[test]
fn removing_accent_lets_go_of_the_node_bound_to_it_and_leaves_bg() {
    let mut a = open();
    a.dispatch(Action::RemoveVariable {
        id: "accent".into(),
        detach: true,
    });
    assert!(refused(&mut a).is_empty());
    assert!(variable(&a, "accent").is_none());
    assert!(variable(&a, "bg").is_some(), "only what was asked goes");
    assert_eq!(node(&a, "dot").fill, Bind::from("#ffffff"));
    assert_eq!(node(&a, "card").fill, Bind::var("bg"));
    a.dispatch(Action::Undo {});
    assert_eq!(node(&a, "dot").fill, Bind::var("accent"));
    assert!(variable(&a, "accent").is_some());
}

#[test]
fn the_core_lets_duplicate_empty_and_odd_names_of_modes_and_collections_through() {
    let mut a = open();
    for name in ["dark", "", "dark\u{202e}x"] {
        a.dispatch(Action::AddMode {
            collection: "t".into(),
            name: name.into(),
            copy_from: None,
        });
        assert!(refused(&mut a).is_empty(), "{name:?}");
    }
    let names: Vec<_> = a
        .vars()
        .collection("t")
        .unwrap()
        .modes
        .iter()
        .map(|m| m.name.clone())
        .collect();
    assert_eq!(names, ["light", "dark", "dark", "", "dark\u{202e}x"]);
    let ids: std::collections::HashSet<_> = a
        .vars()
        .collection("t")
        .unwrap()
        .modes
        .iter()
        .map(|m| m.id.clone())
        .collect();
    assert_eq!(ids.len(), 5, "the ids differ though the names do not");
    // Two collections of one name, too.
    a.dispatch(Action::CreateCollection {
        name: "brand".into(),
    });
    assert!(refused(&mut a).is_empty());
    assert_eq!(
        a.vars()
            .collections
            .iter()
            .filter(|c| c.name == "brand")
            .count(),
        2
    );
}

#[test]
fn a_mode_is_added_to_a_collection_with_no_variables() {
    let mut a = open();
    a.dispatch(Action::CreateCollection {
        name: "empty".into(),
    });
    let id = made(&a.take_events())[0].1.clone();
    a.dispatch(Action::AddMode {
        collection: id.clone(),
        name: "second".into(),
        copy_from: None,
    });
    assert!(refused(&mut a).is_empty());
    assert_eq!(a.vars().collection(&id).unwrap().modes.len(), 2);
}

#[test]
fn a_collection_holds_at_most_sixteen_modes() {
    let mut a = open();
    a.dispatch(Action::CreateCollection {
        name: "many".into(),
    });
    let id = made(&a.take_events())[0].1.clone();
    for _ in 1..16 {
        a.dispatch(Action::AddMode {
            collection: id.clone(),
            name: "m".into(),
            copy_from: None,
        });
        assert!(refused(&mut a).is_empty());
    }
    assert_eq!(a.vars().collection(&id).unwrap().modes.len(), 16);
    let steps = a.state().history.undo_steps;
    a.dispatch(Action::AddMode {
        collection: id.clone(),
        name: "one too many".into(),
        copy_from: None,
    });
    assert_eq!(refused(&mut a), ["E_TOO_MANY_ENTRIES"]);
    assert_eq!(a.vars().collection(&id).unwrap().modes.len(), 16);
    assert_eq!(a.state().history.undo_steps, steps);
}

/// The plan the app makes passes the check; the same plan without the commands that let go of the alias is refused on
/// its last command, after node updates that were fine, and the file is not touched.
#[test]
fn the_check_of_a_group_refuses_it_on_its_last_command_and_applies_nothing() {
    let a = open();
    let plan = removal(a.nodes(), a.vars(), Target::Variable("bg"), true).unwrap();
    assert!(check_group(a.nodes(), a.vars(), &plan).is_ok());
    // `accent` still aliases `bg`: the removal at the end of the group is refused.
    let short: Vec<Command> = plan
        .iter()
        .filter(|c| !matches!(c, Command::SetVariableValue { .. }))
        .cloned()
        .collect();
    assert!(matches!(short.first(), Some(Command::UpdateNode { .. })));
    assert!(matches!(short.last(), Some(Command::RemoveVariable { .. })));
    let before = snapshot(&a);
    let e = check_group(a.nodes(), a.vars(), &short).unwrap_err();
    assert_eq!(e.code(), "E_VARIABLE_IN_USE");
    assert_eq!(snapshot(&a), before);
    // A node update that the file refuses fails the group at once.
    let ghost = Command::UpdateNode {
        id: "ghost".into(),
        patch: NodePatch::default(),
    };
    let e = check_group(a.nodes(), a.vars(), &[ghost]).unwrap_err();
    assert_eq!(e.code(), "E_NODE_NOT_FOUND");
}
