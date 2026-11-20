//! The Variables manage actions: the app mints the ids of a collection, a mode and a variable, and refuses what the core refuses.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, VariableEntity};
use meldframe_protocol::{Bind, DEFAULT_FILL, Value, VarType};

mod common;
use common::*;

fn create(name: &str) -> Action {
    Action::CreateCollection { name: name.into() }
}

fn add_variable(collection: &str, name: &str, kind: VarType) -> Action {
    Action::AddVariable {
        collection: collection.into(),
        name: name.into(),
        kind,
    }
}

#[test]
fn a_collection_gets_one_default_mode_and_an_id_the_app_mints() {
    let mut a = open();
    let steps = a.state().history.undo_steps;
    let fx = a.dispatch(create("spacing"));
    let events = a.take_events();
    let [(VariableEntity::Collection, id)] = &made(&events)[..] else {
        panic!("one collection was added: {events:?}");
    };
    let id = id.clone();
    assert!(id.starts_with("t-1-"), "minted from the prefix: {id}");
    let col = a.vars().collection(&id).expect("the collection is there");
    assert_eq!(col.name, "spacing");
    assert_eq!(col.modes.len(), 1);
    assert_eq!(col.modes[0].name, "default");
    assert_eq!(sent(&fx).len(), 1);
    assert_eq!(a.state().history.undo_steps, steps + 1);
    assert!(
        a.state()
            .variables
            .collections
            .iter()
            .any(|c| c.id == id && c.modes.len() == 1),
        "the panel lists it"
    );
    // One undo step takes it back.
    a.dispatch(Action::Undo {});
    assert!(a.vars().collection(&id).is_none());
}

#[test]
fn two_collections_never_share_an_id() {
    let mut a = open();
    a.dispatch(create("one"));
    a.dispatch(create("two"));
    let ids: Vec<_> = made(&a.take_events()).into_iter().map(|m| m.1).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(a.vars().collections.len(), 4);
}

#[test]
fn a_bad_collection_name_is_refused_with_the_core_code_and_nothing_is_sent() {
    let mut a = open();
    for name in ["", "Brand", "a--b", "a b"] {
        let fx = a.dispatch(create(name));
        assert_eq!(refused(&mut a), ["E_INVALID_NAME"], "{name:?}");
        assert!(sent(&fx).is_empty());
    }
    assert_eq!(a.vars().collections.len(), 2);
    assert!(a.state().pending.is_empty());
}

#[test]
fn a_mode_is_added_last_and_copies_the_default_mode_unless_told_otherwise() {
    let mut a = open();
    a.dispatch(Action::AddMode {
        collection: "t".into(),
        name: "contrast".into(),
        copy_from: None,
    });
    let events = a.take_events();
    let [(VariableEntity::Mode, id)] = &made(&events)[..] else {
        panic!("one mode was added: {events:?}");
    };
    let id = id.clone();
    let col = a.vars().collection("t").unwrap();
    assert_eq!(
        col.modes.last().map(|m| (&m.id, m.name.as_str())),
        Some((&id, "contrast"))
    );
    assert_eq!(
        lit(&variable(&a, "bg").unwrap().values[&id]),
        Value::Color("#ffffff".into())
    );
    a.dispatch(Action::AddMode {
        collection: "t".into(),
        name: "night".into(),
        copy_from: Some("dark".into()),
    });
    let night = made(&a.take_events())[0].1.clone();
    assert_eq!(
        lit(&variable(&a, "bg").unwrap().values[&night]),
        Value::Color("#000000".into())
    );
    // One undo step each.
    a.dispatch(Action::Undo {});
    assert_eq!(a.vars().collection("t").unwrap().modes.len(), 3);
    assert!(!variable(&a, "bg").unwrap().values.contains_key(&night));
}

#[test]
fn a_mode_that_cannot_be_added_is_refused() {
    let mut a = open();
    for (collection, copy_from, code) in [
        ("nope", None, "E_COLLECTION_NOT_FOUND"),
        ("t", Some("nope"), "E_MODE_NOT_FOUND"),
    ] {
        a.dispatch(Action::AddMode {
            collection: collection.into(),
            name: "x".into(),
            copy_from: copy_from.map(Into::into),
        });
        assert_eq!(refused(&mut a), [code]);
    }
    assert_eq!(a.vars().collection("t").unwrap().modes.len(), 2);
    assert!(a.state().pending.is_empty());
}

#[test]
fn a_mode_goes_with_its_values_and_the_default_mode_stays() {
    let mut a = open();
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    a.dispatch(Action::RemoveMode {
        collection: "t".into(),
        mode: "dark".into(),
    });
    assert!(refused(&mut a).is_empty());
    assert_eq!(a.vars().collection("t").unwrap().modes.len(), 1);
    assert!(!variable(&a, "bg").unwrap().values.contains_key("dark"));
    assert!(
        a.state().modes.is_empty(),
        "the pick of a removed mode goes back to the default"
    );
    a.dispatch(Action::Undo {});
    assert!(variable(&a, "bg").unwrap().values.contains_key("dark"));
    for (collection, mode, code) in [
        ("t", "light", "E_DEFAULT_MODE"),
        ("t", "nope", "E_MODE_NOT_FOUND"),
        ("nope", "dark", "E_COLLECTION_NOT_FOUND"),
    ] {
        a.dispatch(Action::RemoveMode {
            collection: collection.into(),
            mode: mode.into(),
        });
        assert_eq!(refused(&mut a), [code], "{collection} {mode}");
    }
}

#[test]
fn a_variable_starts_with_a_neutral_value_in_every_mode() {
    let mut a = open();
    a.dispatch(add_variable("t", "color/ink", VarType::Color));
    a.dispatch(add_variable("t", "space/gap", VarType::Number));
    let ids: Vec<_> = made(&a.take_events()).into_iter().map(|m| m.1).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|i| i.starts_with("t-1-")));
    let ink = variable(&a, &ids[0]).unwrap();
    assert_eq!((ink.name.as_str(), ink.kind), ("color/ink", VarType::Color));
    for mode in ["light", "dark"] {
        assert_eq!(lit(&ink.values[mode]), Value::Color(DEFAULT_FILL.into()));
        assert_eq!(
            variable(&a, &ids[1]).unwrap().values[mode],
            Bind::Lit(Value::Number(0.0))
        );
    }
    assert_eq!(variable(&a, &ids[1]).unwrap().values.len(), 2);
    assert!(
        a.state()
            .variables
            .collections
            .iter()
            .flat_map(|c| &c.variables)
            .any(|v| v.id == ids[0])
    );
}

#[test]
fn a_variable_that_cannot_be_added_is_refused_with_the_core_code() {
    let mut a = open();
    for (collection, name, code) in [
        ("t", "Color/Ink", "E_INVALID_NAME"),
        ("t", "color/bg", "E_DUPLICATE_NAME"),
        ("nope", "color/ink", "E_COLLECTION_NOT_FOUND"),
    ] {
        let fx = a.dispatch(add_variable(collection, name, VarType::Color));
        assert_eq!(refused(&mut a), [code], "{name}");
        assert!(sent(&fx).is_empty());
    }
    assert_eq!(a.vars().variables.len(), 3);
    // The same name in another collection is fine.
    a.dispatch(add_variable("u", "color/bg", VarType::Color));
    assert!(refused(&mut a).is_empty());
}

#[test]
fn the_actions_have_a_json_shape_without_ids() {
    for (json, ok) in [
        (r#"{"type":"createCollection","name":"x"}"#, true),
        (r#"{"type":"addMode","collection":"t","name":"x"}"#, true),
        (
            r#"{"type":"addMode","collection":"t","name":"x","copyFrom":"dark"}"#,
            true,
        ),
        (
            r#"{"type":"removeMode","collection":"t","mode":"dark"}"#,
            true,
        ),
        (
            r#"{"type":"addVariable","collection":"t","name":"a/b","kind":"number"}"#,
            true,
        ),
        (r#"{"type":"removeVariable","id":"bg","detach":true}"#, true),
        (
            r#"{"type":"removeCollection","id":"t","detach":false}"#,
            true,
        ),
        (
            r#"{"type":"createCollection","name":"x","id":"mine"}"#,
            false,
        ),
        (r#"{"type":"removeVariable","id":"bg"}"#, false),
    ] {
        assert_eq!(serde_json::from_str::<Action>(json).is_ok(), ok, "{json}");
    }
}
