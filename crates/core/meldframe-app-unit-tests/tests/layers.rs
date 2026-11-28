//! `AppState::layers`: the Layers panel as rows (rule U9 gives the names; the tree is what the web client built from the
//! nodes and the scene until now): front node first, children under their parent, a collapsed node hiding what is
//! below it, the selection and the last actor on each row, nothing while the panel is closed.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, App, Panel};
use meldframe_protocol::{Command, Node, NodePatch};
use serde_json::json;

fn file() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "page", "name": "Page", "type": "frame", "pos": "a0"},
        {"id": "back", "name": "Back", "type": "rect", "parentId": "page", "pos": "a0"},
        {"id": "card", "name": "Card", "type": "frame", "parentId": "page", "pos": "a1"},
        {"id": "title", "name": "Title", "type": "text", "parentId": "card", "pos": "a0"},
        {"id": "side", "name": "Side", "type": "rect", "pos": "a1"},
    ]))
    .unwrap()
}

fn app() -> App {
    App::new("user:me", "c0", 1, file(), 0).unwrap()
}

fn shape(a: &App) -> Vec<(String, u32)> {
    let rows = &a.state().layers;
    rows.iter().map(|r| (r.id.clone(), r.depth)).collect()
}

fn row(id: &str, depth: u32) -> (String, u32) {
    (id.to_string(), depth)
}

#[test]
fn the_rows_run_front_first_with_children_under_their_parent() {
    let a = app();
    // The last sibling is the front one.
    assert_eq!(
        shape(&a),
        [
            row("side", 0),
            row("page", 0),
            row("card", 1),
            row("title", 2),
            row("back", 1)
        ]
    );
    let page = &a.state().layers[1];
    assert_eq!(
        (page.has_children, page.collapsed, page.selected),
        (true, false, false)
    );
    assert!(!a.state().layers[0].has_children);
}

#[test]
fn a_collapsed_node_keeps_its_row_and_hides_what_is_below_it() {
    let mut a = app();
    a.dispatch(Action::ToggleCollapsed { id: "card".into() });
    assert_eq!(
        shape(&a),
        [
            row("side", 0),
            row("page", 0),
            row("card", 1),
            row("back", 1)
        ]
    );
    assert!(a.state().layers[2].collapsed);
    a.dispatch(Action::ToggleCollapsed { id: "card".into() });
    assert_eq!(a.state().layers.len(), 5);
}

#[test]
fn the_selection_and_the_last_actor_are_on_the_rows() {
    let mut l = Loopback::live();
    l.app = app();
    l.app.dispatch(Action::Connected { now_ms: 0 });
    l.app.dispatch(Action::Select {
        ids: ids(&["back"]),
    });
    let patch = NodePatch {
        x: Some(5.0),
        ..Default::default()
    };
    l.remote(
        "agent:x",
        "b1:0",
        1,
        Command::UpdateNode {
            id: "back".into(),
            patch,
        },
    );
    let rows = &l.app.state().layers;
    let back = rows.iter().find(|r| r.id == "back").unwrap();
    assert_eq!(
        (back.selected, back.actor.as_ref().map(|v| v.actor.as_str())),
        (true, Some("agent:x"))
    );
    let page = rows.iter().find(|r| r.id == "page").unwrap();
    assert_eq!((page.selected, page.actor.as_ref()), (false, None));
}

#[test]
fn nothing_is_listed_while_the_panel_is_closed() {
    let mut a = app();
    a.dispatch(Action::SetPanel {
        panel: Panel::Layers,
        open: false,
    });
    assert!(a.state().layers.is_empty());
    a.dispatch(Action::SetPanel {
        panel: Panel::Layers,
        open: true,
    });
    assert_eq!(a.state().layers.len(), 5);
}

#[test]
fn a_node_whose_parent_is_not_in_the_file_is_a_node_of_the_page_and_a_cycle_has_no_row() {
    // A file that loads has neither; the function is held to the render step's reading all the same.
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "lost", "name": "Lost", "type": "rect", "parentId": "gone"},
        {"id": "a", "name": "A", "type": "rect", "parentId": "b"},
        {"id": "b", "name": "B", "type": "rect", "parentId": "a"},
    ]))
    .unwrap();
    let rows = meldframe_app::parts::layers::layer_rows(&nodes, &[], &[], &Default::default());
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["lost"]);
}

#[test]
fn a_node_added_or_removed_changes_the_rows_at_once() {
    let mut l = Loopback::live();
    l.app = app();
    l.app.dispatch(Action::Connected { now_ms: 0 });
    l.send(Action::Delete {
        ids: ids(&["side"]),
    });
    assert_eq!(l.app.state().layers.len(), 4);
}
