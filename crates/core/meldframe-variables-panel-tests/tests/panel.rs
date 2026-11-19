//! E3.4: the rules of the Variables panel live in the app layer. The list and its search, the value of each mode,
//! the bindings of the selected node and the edits are state and actions of `meldframe-app`; a front end draws
//! the rows and sends the actions.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{
    Action, App, BindField, BindOption, Event, Panel, Property, VariableRow, VariableValue,
};
use meldframe_protocol::{Doc, Node};
use serde_json::json;

mod common;
use common::{appearance, bg, cmd};

fn round() -> meldframe_protocol::Command {
    cmd(
        json!({"type": "addVariable", "variable": {"id": "round", "collection": "t", "name": "radius/card", "type": "number",
        "values": {"light": 4, "dark": 12}}}),
    )
}

/// A frame `card` with its fill bound to `bg`, a plain rectangle and a text node; `bg` and `round` in `t`.
fn file() -> Doc {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "card", "type": "rect", "pos": "a0", "fill": {"var": "bg"}},
        {"id": "plain", "type": "rect", "pos": "a1"},
        {"id": "label", "type": "text", "pos": "a2"},
    ]))
    .unwrap();
    let mut d = Doc::of_nodes(nodes);
    for c in [appearance(), bg(), round()] {
        meldframe_core::apply_doc(&mut d, &c, &Default::default()).unwrap();
    }
    d
}

fn open() -> App {
    let mut a = App::with_doc("user:a", "t", 1, file(), 0).unwrap();
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: true,
    });
    a.take_events();
    a
}

fn row<'a>(a: &'a App, id: &str) -> &'a VariableRow {
    a.state()
        .variables
        .collections
        .iter()
        .flat_map(|c| &c.variables)
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no row {id}: {:?}", a.state().variables))
}

fn texts(r: &VariableRow) -> Vec<(&str, &str)> {
    r.values
        .iter()
        .map(|v| (v.mode.as_str(), v.text.as_str()))
        .collect()
}

fn select(a: &mut App, id: &str) {
    a.dispatch(Action::Select {
        ids: vec![id.into()],
    });
}

fn set(variable: &str, mode: &str, text: &str) -> Action {
    Action::SetVariableValue {
        variable: variable.into(),
        mode: mode.into(),
        value: VariableValue::Literal { text: text.into() },
    }
}

fn refused(a: &mut App) -> Vec<String> {
    a.take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect()
}

#[test]
fn the_panel_lists_nothing_until_it_is_open() {
    let mut a = App::with_doc("user:a", "t", 1, file(), 0).unwrap();
    assert!(!a.state().panels.variables);
    assert!(a.state().variables.collections.is_empty());
    a.dispatch(Action::TogglePanel {
        panel: Panel::Variables,
    });
    assert!(a.state().panels.variables);
    assert_eq!(a.state().variables.total, 2);
    a.dispatch(Action::TogglePanel {
        panel: Panel::Variables,
    });
    assert!(a.state().variables.collections.is_empty());
}

#[test]
fn it_lists_collections_modes_values_and_how_much_each_variable_is_used() {
    let a = open();
    let v = &a.state().variables;
    assert_eq!((v.total, v.matched), (2, 2));
    let c = &v.collections[0];
    assert_eq!((c.id.as_str(), c.name.as_str()), ("t", "appearance"));
    assert_eq!(
        c.modes.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["light", "dark"]
    );
    assert_eq!(c.active, "light");
    let bg = row(&a, "bg");
    assert_eq!(bg.name, "color/bg");
    assert_eq!(texts(bg), [("light", "#ffffff"), ("dark", "#000000")]);
    assert_eq!(bg.used_by, 1, "the fill of card");
    assert_eq!(row(&a, "round").used_by, 0);
    assert_eq!(texts(row(&a, "round")), [("light", "4"), ("dark", "12")]);
}

#[test]
fn an_alias_shows_the_name_it_points_at_and_what_it_resolves_to_in_each_mode() {
    let mut a = open();
    a.dispatch(Action::EditVariables {
        commands: vec![cmd(
            json!({"type": "addVariable", "variable": {"id": "panel", "collection": "t", "name": "color/panel", "type": "color",
            "values": {"light": {"var": "bg"}, "dark": "#222222"}}}),
        )],
    });
    let r = row(&a, "panel");
    assert_eq!(texts(r), [("light", "color/bg"), ("dark", "#222222")]);
    assert_eq!(r.values[0].alias.as_deref(), Some("bg"));
    assert_eq!(r.values[0].resolved, "#ffffff");
    assert_eq!(r.values[1].alias, None);
    assert_eq!(r.values[1].resolved, "#222222");
    assert_eq!(row(&a, "bg").used_by, 2, "the fill of card and the alias");
}

#[test]
fn the_active_mode_follows_the_switch_and_the_document_does_not_change() {
    let mut a = open();
    let nodes = serde_json::to_string(a.nodes()).unwrap();
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert_eq!(a.state().variables.collections[0].active, "dark");
    assert_eq!(a.scene()[0].fill, Some(0x000000));
    assert_eq!(serde_json::to_string(a.nodes()).unwrap(), nodes);
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: None,
    });
    assert_eq!(a.state().variables.collections[0].active, "light");
}

#[test]
fn the_search_matches_names_and_values_and_says_what_it_left_out() {
    let mut a = open();
    a.dispatch(Action::SetVariablesQuery {
        query: "  BG ".into(),
    });
    assert_eq!(
        a.take_events(),
        [Event::VariablesQueryChanged { query: "BG".into() }]
    );
    let v = &a.state().variables;
    assert_eq!((v.query.as_str(), v.total, v.matched), ("BG", 2, 1));
    assert_eq!(v.collections[0].variables.len(), 1);
    // A value, and every word must match.
    a.dispatch(Action::SetVariablesQuery {
        query: "#000000".into(),
    });
    assert_eq!(a.state().variables.collections[0].variables[0].id, "bg");
    a.dispatch(Action::SetVariablesQuery {
        query: "color dark".into(),
    });
    assert_eq!(a.state().variables.matched, 0, "no variable has both words");
    assert!(a.state().variables.collections.is_empty());
    // The collection name matches all its variables.
    a.dispatch(Action::SetVariablesQuery {
        query: "appearance".into(),
    });
    assert_eq!(a.state().variables.matched, 2);
    // Clearing shows everything; the same text again raises no event.
    a.dispatch(Action::SetVariablesQuery { query: "".into() });
    assert_eq!(a.state().variables.matched, 2);
    a.take_events();
    a.dispatch(Action::SetVariablesQuery { query: "".into() });
    assert!(a.take_events().is_empty());
}

#[test]
fn a_long_list_is_cut_and_the_counts_tell_so() {
    let mut d = file();
    for i in 0..250 {
        let c = cmd(
            json!({"type": "addVariable", "variable": {"id": format!("n{i}"), "collection": "t", "name": format!("number/n{i}"),
            "type": "number", "values": {"light": i, "dark": i}}}),
        );
        meldframe_core::apply_doc(&mut d, &c, &Default::default()).unwrap();
    }
    let mut a = App::with_doc("user:a", "t", 1, d, 0).unwrap();
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: true,
    });
    let v = &a.state().variables;
    let listed: usize = v.collections.iter().map(|c| c.variables.len()).sum();
    assert_eq!((v.total, v.matched, listed), (252, 252, 200));
    a.dispatch(Action::SetVariablesQuery {
        query: "n249".into(),
    });
    assert_eq!(a.state().variables.matched, 1);
}

#[test]
fn editing_a_value_is_one_undo_step_and_the_canvas_repaints() {
    let mut a = open();
    a.dispatch(set("bg", "light", "#EEEEEE"));
    assert!(refused(&mut a).is_empty());
    assert_eq!(texts(row(&a, "bg"))[0], ("light", "#EEEEEE"));
    assert_eq!(a.scene()[0].fill, Some(0xeeeeee));
    assert_eq!(a.state().history.undo_steps, 1);
    a.dispatch(Action::Undo {});
    assert_eq!(texts(row(&a, "bg"))[0], ("light", "#ffffff"));
    // A number is typed as text.
    a.dispatch(set("round", "dark", " 8.5 "));
    assert_eq!(texts(row(&a, "round"))[1], ("dark", "8.5"));
}

#[test]
fn a_value_that_does_not_change_makes_no_step() {
    let mut a = open();
    a.dispatch(set("bg", "light", "#ffffff"));
    a.dispatch(set("round", "light", "4"));
    assert_eq!(a.state().history.undo_steps, 0);
    assert!(a.take_events().is_empty());
}

#[test]
fn a_value_that_cannot_be_set_is_refused_with_a_code_and_nothing_changes() {
    let mut a = open();
    for (action, code) in [
        (set("round", "light", "wide"), "invalid_value"),
        (set("round", "light", ""), "invalid_value"),
        (set("bg", "light", "red"), "E_INVALID_COLOR"),
        (set("nope", "light", "#fff"), "E_VARIABLE_NOT_FOUND"),
        (set("bg", "nope", "#fff"), "E_MODE_NOT_FOUND"),
    ] {
        a.dispatch(action);
        assert_eq!(refused(&mut a), [code]);
    }
    assert_eq!(a.state().history.undo_steps, 0);
    assert_eq!(texts(row(&a, "bg"))[0], ("light", "#ffffff"));
}

#[test]
fn a_value_can_become_an_alias_and_a_cycle_or_a_wrong_type_is_refused() {
    let mut a = open();
    a.dispatch(Action::EditVariables {
        commands: vec![cmd(
            json!({"type": "addVariable", "variable": {"id": "panel", "collection": "t", "name": "color/panel", "type": "color",
            "values": {"light": "#111111", "dark": "#222222"}}}),
        )],
    });
    let alias = |v: &str, to: &str| Action::SetVariableValue {
        variable: v.into(),
        mode: "dark".into(),
        value: VariableValue::Alias {
            variable: to.into(),
        },
    };
    a.dispatch(alias("panel", "bg"));
    assert!(refused(&mut a).is_empty());
    let v = &row(&a, "panel").values[1];
    assert_eq!(
        (v.alias.as_deref(), v.resolved.as_str()),
        (Some("bg"), "#000000")
    );
    a.dispatch(alias("bg", "panel"));
    assert_eq!(refused(&mut a), ["E_ALIAS_CYCLE"]);
    a.dispatch(alias("panel", "round"));
    assert_eq!(refused(&mut a), ["E_TYPE_MISMATCH"]);
    a.dispatch(alias("panel", "missing"));
    assert_eq!(refused(&mut a), ["E_VARIABLE_NOT_FOUND"]);
}

fn binds(a: &App, id: &str) -> Vec<BindOption> {
    row(a, id).binds.clone()
}

fn opt(field: BindField, bound: bool) -> BindOption {
    BindOption { field, bound }
}

#[test]
fn a_row_offers_the_fields_of_the_selected_node_that_fit_its_type() {
    let mut a = open();
    assert!(binds(&a, "bg").is_empty(), "nothing is selected");
    select(&mut a, "plain");
    assert_eq!(binds(&a, "bg"), [opt(BindField::Fill, false)]);
    assert_eq!(
        binds(&a, "round"),
        [
            opt(BindField::Radius, false),
            opt(BindField::Opacity, false)
        ]
    );
    select(&mut a, "card");
    assert_eq!(binds(&a, "bg"), [opt(BindField::Fill, true)]);
    // A text node has no radius.
    select(&mut a, "label");
    assert_eq!(binds(&a, "round"), [opt(BindField::Opacity, false)]);
    a.dispatch(Action::Select {
        ids: vec!["card".into(), "plain".into()],
    });
    assert!(binds(&a, "bg").is_empty(), "two nodes are selected");
}

#[test]
fn binding_a_field_paints_the_variable_and_one_undo_takes_it_back() {
    let mut a = open();
    select(&mut a, "plain");
    a.dispatch(Action::BindProperty {
        id: "plain".into(),
        field: BindField::Radius,
        variable: Some("round".into()),
    });
    assert!(refused(&mut a).is_empty());
    assert_eq!(a.scene()[1].radius, 4.0);
    assert_eq!(binds(&a, "round")[0], opt(BindField::Radius, true));
    assert_eq!(row(&a, "round").used_by, 1);
    assert_eq!(
        a.state().properties.as_ref().unwrap().bound,
        ["radius".to_string()]
    );
    // The mode switch reaches the bound node.
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert_eq!(a.scene()[1].radius, 12.0);
    a.dispatch(Action::Undo {});
    assert_eq!(binds(&a, "round")[0], opt(BindField::Radius, false));
    assert_eq!(a.scene()[1].radius, 0.0);
}

#[test]
fn a_binding_of_the_wrong_type_or_to_nothing_is_refused() {
    let mut a = open();
    for (field, var, code) in [
        (BindField::Fill, "round", "E_TYPE_MISMATCH"),
        (BindField::Radius, "bg", "E_TYPE_MISMATCH"),
        (BindField::Opacity, "nope", "E_VARIABLE_NOT_FOUND"),
    ] {
        a.dispatch(Action::BindProperty {
            id: "plain".into(),
            field,
            variable: Some(var.into()),
        });
        assert_eq!(refused(&mut a), [code]);
    }
    a.dispatch(Action::BindProperty {
        id: "gone".into(),
        field: BindField::Fill,
        variable: Some("bg".into()),
    });
    assert_eq!(refused(&mut a), ["not_found"]);
    a.dispatch(Action::BindProperty {
        id: "label".into(),
        field: BindField::Radius,
        variable: Some("round".into()),
    });
    assert_eq!(refused(&mut a), ["not_bindable"]);
    assert_eq!(a.state().history.undo_steps, 0);
}

#[test]
fn unbinding_keeps_what_is_painted_in_the_mode_on_view() {
    let mut a = open();
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    a.dispatch(Action::BindProperty {
        id: "card".into(),
        field: BindField::Fill,
        variable: None,
    });
    assert!(refused(&mut a).is_empty());
    select(&mut a, "card");
    let p = a.state().properties.clone().unwrap();
    assert_eq!((p.fill.as_str(), p.bound.len()), ("#000000", 0));
    assert_eq!(row(&a, "bg").used_by, 0);
    // The field is a literal now: the mode no longer changes it.
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: None,
    });
    assert_eq!(a.scene()[0].fill, Some(0x000000));
    // Unbinding a field that is not bound changes nothing.
    a.take_events();
    let steps = a.state().history.undo_steps;
    a.dispatch(Action::BindProperty {
        id: "card".into(),
        field: BindField::Fill,
        variable: None,
    });
    assert_eq!(a.state().history.undo_steps, steps);
    // A literal typed in the properties panel replaces a binding too.
    a.dispatch(Action::Undo {});
    a.dispatch(Action::SetProperty {
        id: "card".into(),
        property: Property::Fill("#123456".into()),
    });
    assert_eq!(row(&a, "bg").used_by, 0);
}

#[test]
fn a_change_from_another_actor_updates_the_rows() {
    let mut a = open();
    a.dispatch(Action::Start { now_ms: 0 });
    a.dispatch(Action::Connected { now_ms: 0 });
    a.dispatch(Action::Frame {
        now_ms: 1,
        frame: meldframe_app::Frame::Event {
            seq: 1,
            actor: "user:b".into(),
            key: None,
            command: common::set_bg("dark", "#101010"),
        },
    });
    assert_eq!(texts(row(&a, "bg"))[1], ("dark", "#101010"));
}
