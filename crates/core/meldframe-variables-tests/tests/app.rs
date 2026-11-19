//! E3.3: the app layer paints resolved variables and switches the mode as state that is not the document.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, Effect, Event, Frame};
use meldframe_protocol::{Doc, Node};
use serde_json::json;

mod common;
use common::{appearance, bg, cmd, set_bg};

fn file() -> Doc {
    let node: Node = serde_json::from_value(
        json!({"id": "card", "type": "rect", "pos": "a0", "fill": {"var": "bg"}}),
    )
    .unwrap();
    let mut d = Doc::of_nodes(vec![node]);
    for c in [appearance(), bg()] {
        meldframe_core::apply_doc(&mut d, &c, &Default::default()).unwrap();
    }
    d
}

fn app() -> App {
    let mut a = App::with_doc("user:a", "t", 1, file(), 0).unwrap();
    a.take_events();
    a
}

fn fill(a: &App) -> Option<u32> {
    a.scene()[0].fill
}

#[test]
fn the_scene_paints_the_default_mode_and_the_document_keeps_the_binding() {
    let a = app();
    assert_eq!(fill(&a), Some(0xffffff));
    assert_eq!(
        serde_json::to_value(&a.nodes()[0]).unwrap()["fill"],
        json!({"var": "bg"})
    );
}

#[test]
fn switching_the_mode_repaints_without_a_change_to_the_document() {
    let mut a = app();
    a.dispatch(Action::Start { now_ms: 0 });
    a.dispatch(Action::Connected { now_ms: 0 });
    a.take_events();
    let nodes = serde_json::to_string(a.nodes()).unwrap();
    let steps = a.state().history.undo_steps;
    let fx = a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert_eq!(fill(&a), Some(0x000000));
    assert_eq!(a.state().modes.get("t").map(String::as_str), Some("dark"));
    assert!(
        fx.iter()
            .any(|e| matches!(e, Effect::Render { nodes } if nodes.is_empty())),
        "{fx:?}"
    );
    assert!(
        !fx.iter().any(|e| matches!(e, Effect::SendOps { .. })),
        "no op is sent: {fx:?}"
    );
    assert_eq!(
        a.take_events(),
        [Event::ModeChanged {
            collection: "t".into(),
            mode: Some("dark".into())
        }]
    );
    assert_eq!(serde_json::to_string(a.nodes()).unwrap(), nodes);
    assert_eq!(
        (a.state().history.undo_steps, a.state().pending.len()),
        (steps, 0)
    );
    // The default mode again.
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: None,
    });
    assert_eq!(
        (fill(&a), a.state().modes.is_empty()),
        (Some(0xffffff), true)
    );
}

#[test]
fn a_mode_that_does_not_exist_is_refused_and_nothing_changes() {
    let mut a = app();
    for (c, m, code) in [
        ("nope", "dark", "E_COLLECTION_NOT_FOUND"),
        ("t", "nope", "E_MODE_NOT_FOUND"),
    ] {
        let fx = a.dispatch(Action::SetMode {
            collection: c.into(),
            mode: Some(m.into()),
        });
        assert!(fx.is_empty());
        assert_eq!(a.take_events(), [Event::Refused { code: code.into() }]);
        assert!(a.state().modes.is_empty());
    }
}

#[test]
fn a_variable_command_from_the_server_repaints_everything() {
    let mut a = app();
    a.dispatch(Action::Start { now_ms: 0 });
    a.dispatch(Action::Connected { now_ms: 0 });
    let fx = a.dispatch(Action::Frame {
        now_ms: 1,
        frame: Frame::Event {
            seq: 1,
            actor: "user:b".into(),
            key: None,
            command: set_bg("light", "#eeeeee"),
        },
    });
    assert_eq!(fill(&a), Some(0xeeeeee));
    assert!(
        fx.iter()
            .any(|e| matches!(e, Effect::Render { nodes } if nodes.is_empty())),
        "{fx:?}"
    );
}

#[test]
fn my_own_variable_command_is_sent_like_a_node_command_and_undone() {
    let mut a = app();
    a.dispatch(Action::Start { now_ms: 0 });
    a.dispatch(Action::Connected { now_ms: 0 });
    let fx = a.dispatch(Action::EditVariables {
        commands: vec![set_bg("light", "#eeeeee")],
    });
    assert_eq!(fill(&a), Some(0xeeeeee));
    assert!(
        fx.iter()
            .any(|e| matches!(e, Effect::SendOps { ops } if ops.len() == 1)),
        "{fx:?}"
    );
    a.dispatch(Action::Undo {});
    assert_eq!(fill(&a), Some(0xffffff));
}

#[test]
fn a_list_of_variable_commands_is_checked_in_order_and_is_one_undo_step() {
    let mut a = App::new("user:a", "t", 1, vec![], 0).unwrap();
    let steps = a.state().history.undo_steps;
    // The variable needs the collection that comes before it in the same list.
    a.dispatch(Action::EditVariables {
        commands: vec![appearance(), bg(), set_bg("dark", "#111111")],
    });
    assert_eq!(a.vars().variables.len(), 1);
    assert_eq!(a.state().history.undo_steps, steps + 1);
    a.dispatch(Action::Undo {});
    assert!(a.vars().is_empty(), "one undo takes the whole list back");
}

#[test]
fn a_list_that_fails_half_way_applies_nothing_and_names_the_code() {
    let mut a = app();
    let bad = cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": {"var": "bg"}}),
    );
    a.dispatch(Action::EditVariables {
        commands: vec![set_bg("dark", "#111111"), bad],
    });
    assert_eq!(
        a.take_events(),
        [Event::Refused {
            code: "E_ALIAS_CYCLE".into()
        }]
    );
    assert_eq!(a.state().pending.len(), 0);
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert_eq!(
        fill(&a),
        Some(0x000000),
        "the first command was not applied"
    );
    // A command about nodes does not belong in this action.
    a.take_events();
    a.dispatch(Action::EditVariables {
        commands: vec![common::add("x", "rect", None)],
    });
    assert_eq!(
        a.take_events(),
        [Event::Refused {
            code: "not_a_variable_command".into()
        }]
    );
}

#[test]
fn the_properties_show_the_resolved_fill_and_name_the_bound_fields() {
    let mut a = app();
    a.dispatch(Action::Select {
        ids: vec!["card".into()],
    });
    let p = a.state().properties.clone().unwrap();
    assert_eq!(
        (p.fill.as_str(), p.bound.as_slice()),
        ("#ffffff", ["fill".to_string()].as_slice())
    );
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert_eq!(a.state().properties.clone().unwrap().fill, "#000000");
}

#[test]
fn a_reload_takes_the_variables_of_the_new_snapshot() {
    let mut a = app();
    let mut d = file();
    meldframe_core::apply_doc(&mut d, &set_bg("light", "#abcdef"), &Default::default()).unwrap();
    a.dispatch(Action::Reload {
        now_ms: 5,
        nodes: d.nodes.clone(),
        variables: d.vars.clone(),
        seq: 3,
        applied: vec![],
    });
    assert_eq!(fill(&a), Some(0xabcdef));
}

#[test]
fn an_empty_list_of_variable_commands_leaves_no_undo_step() {
    let mut a = app();
    let steps = a.state().history.undo_steps;
    let can_undo = a.state().history.can_undo;
    a.dispatch(Action::EditVariables { commands: vec![] });
    assert_eq!(
        (a.state().history.undo_steps, a.state().history.can_undo),
        (steps, can_undo)
    );
    assert_eq!(a.state().pending.len(), 0);
}

#[test]
fn the_properties_show_what_the_canvas_paints_for_a_number_the_field_cannot_hold() {
    let node: Node = serde_json::from_value(
        json!({"id": "card", "type": "rect", "pos": "a0", "radius": {"var": "round"}}),
    )
    .unwrap();
    let mut d = Doc::of_nodes(vec![node]);
    for c in [
        appearance(),
        cmd(
            json!({"type": "addVariable", "variable": {"id": "round", "collection": "t", "name": "radius/card", "type": "number",
            "values": {"light": -4, "dark": 12}}}),
        ),
    ] {
        meldframe_core::apply_doc(&mut d, &c, &Default::default()).unwrap();
    }
    let mut a = App::with_doc("user:a", "t", 1, d, 0).unwrap();
    a.dispatch(Action::Select {
        ids: vec!["card".into()],
    });
    let painted = a.scene()[0].radius;
    assert_eq!(painted, 0.0, "the canvas holds a radius to zero or more");
    assert_eq!(a.state().properties.clone().unwrap().radius, painted);
}
