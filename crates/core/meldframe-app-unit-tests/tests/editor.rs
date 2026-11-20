//! Selection, tool and panel rules, driven without a front end.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, AppState, Connection, Event, Panel, Tool};

#[test]
fn a_new_app_has_the_default_state_and_no_events() {
    let mut app = app();
    let s = app.state();
    assert!(s.selection.is_empty() && s.properties.is_none());
    assert_eq!(s.tool, Tool::Select);
    assert!(s.panels.layers && s.panels.properties && s.panels.collapsed.is_empty());
    assert_eq!(s.connection, Connection::Connecting);
    assert!(!s.history.can_undo && !s.history.can_redo);
    assert!(s.pending.is_empty() && s.last_actor.is_empty());
    assert!(app.take_events().is_empty());
    assert_eq!(app.state().clone(), app.state().clone());
    let _: AppState = AppState::default();
}

#[test]
fn select_keeps_known_nodes_once_and_raises_one_event() {
    let mut app = app();
    assert!(
        app.dispatch(Action::Select {
            ids: ids(&["r1", "nope", "t1", "r1"])
        })
        .is_empty()
    );
    assert_eq!(app.state().selection, ids(&["r1", "t1"]));
    assert_eq!(
        app.take_events(),
        vec![Event::SelectionChanged {
            ids: ids(&["r1", "t1"])
        }]
    );
    app.dispatch(Action::Select {
        ids: ids(&["r1", "t1"]),
    });
    assert!(app.take_events().is_empty(), "no change, no event");
}

#[test]
fn toggle_adds_removes_and_ignores_an_unknown_node() {
    let mut app = app();
    app.dispatch(Action::ToggleSelect { id: "r1".into() });
    app.dispatch(Action::ToggleSelect { id: "t1".into() });
    app.dispatch(Action::ToggleSelect { id: "ghost".into() });
    assert_eq!(app.state().selection, ids(&["r1", "t1"]));
    app.dispatch(Action::ToggleSelect { id: "r1".into() });
    assert_eq!(app.state().selection, ids(&["t1"]));
    app.dispatch(Action::ClearSelection {});
    assert!(app.state().selection.is_empty());
}

#[test]
fn the_properties_panel_follows_a_single_selection_only() {
    let mut app = app();
    app.dispatch(Action::Select { ids: ids(&["t1"]) });
    let p = app.state().properties.clone().unwrap();
    assert_eq!((p.id.as_str(), p.w, p.h), ("t1", 100.0, 100.0));
    app.dispatch(Action::Select {
        ids: ids(&["t1", "r1"]),
    });
    assert!(app.state().properties.is_none());
}

#[test]
fn the_tool_changes_and_raises_an_event_only_on_change() {
    let mut app = app();
    app.dispatch(Action::SetTool { tool: Tool::Frame });
    app.dispatch(Action::SetTool { tool: Tool::Frame });
    assert_eq!(app.state().tool, Tool::Frame);
    assert_eq!(
        app.take_events(),
        vec![Event::ToolChanged { tool: Tool::Frame }]
    );
}

#[test]
fn panels_open_close_and_layers_collapse() {
    let mut app = app();
    app.dispatch(Action::TogglePanel {
        panel: Panel::Layers,
    });
    app.dispatch(Action::SetPanel {
        panel: Panel::Properties,
        open: false,
    });
    assert!(!app.state().panels.layers && !app.state().panels.properties);
    app.dispatch(Action::ToggleCollapsed { id: "root".into() });
    assert_eq!(app.state().panels.collapsed, ids(&["root"]));
    app.dispatch(Action::ToggleCollapsed { id: "root".into() });
    assert!(app.state().panels.collapsed.is_empty());
    let ev = app.take_events();
    assert!(ev.contains(&Event::PanelChanged {
        panel: Panel::Layers,
        open: false
    }));
    assert!(ev.contains(&Event::CollapsedChanged {
        id: "root".into(),
        collapsed: true
    }));
}

#[test]
fn the_scene_is_the_render_step_over_the_document_as_the_person_sees_it() {
    let mut l = Loopback::live();
    let ids: Vec<_> = l.app.scene().into_iter().map(|b| b.id).collect();
    assert_eq!(ids, ["root", "r1", "t1"]);
    l.send(Action::Move {
        id: "r1".into(),
        x: 7.0,
        y: 8.0,
    });
    let b = l.app.scene().into_iter().find(|b| b.id == "r1").unwrap();
    assert_eq!((b.x, b.y, b.parent.as_deref()), (7.0, 8.0, Some("root")));
    l.send(Action::Undo {});
    let b = l.app.scene().into_iter().find(|b| b.id == "r1").unwrap();
    assert_eq!((b.x, b.y), (0.0, 0.0));
}
