//! E3.4: the edges of the Variables panel at the app seam: when the view is built, the cap, the search text, a
//! radius that cannot be bound, and what another actor's changes do to the rows.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, BindField, Panel};
use meldframe_protocol::{Doc, Node};
use serde_json::json;

mod common;
use common::*;

fn builds(a: &App) -> u64 {
    a.variables_view_builds()
}

#[test]
fn the_view_is_not_rebuilt_by_a_dispatch_that_changes_nothing_it_reads() {
    let mut a = open();
    select(&mut a, "plain");
    let b = builds(&a);
    a.dispatch(Action::Move {
        id: "plain".into(),
        x: 5.0,
        y: 6.0,
    });
    a.dispatch(Action::Resize {
        id: "plain".into(),
        w: 50.0,
        h: 60.0,
        x: None,
        y: None,
    });
    a.dispatch(Action::SetTool {
        tool: meldframe_app::Tool::Rectangle,
    });
    a.dispatch(Action::Tick { now_ms: 1 });
    assert_eq!(builds(&a), b, "a move or a resize is not read by the panel");
    // What it reads: the selection, the modes, the query, the bindings and the variables.
    select(&mut a, "card");
    assert_eq!(builds(&a), b + 1);
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert_eq!(builds(&a), b + 2);
    a.dispatch(Action::SetVariablesQuery { query: "bg".into() });
    assert_eq!(builds(&a), b + 3);
    a.dispatch(set("bg", "dark", "#123456"));
    assert_eq!(builds(&a), b + 4);
    a.dispatch(Action::BindProperty {
        id: "plain".into(),
        field: BindField::Fill,
        variable: Some("bg".into()),
    });
    assert_eq!(
        builds(&a),
        b + 5,
        "a binding of another node changes a use count"
    );
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: false,
    });
    a.dispatch(Action::Move {
        id: "plain".into(),
        x: 7.0,
        y: 7.0,
    });
    assert_eq!(builds(&a), b + 5, "a closed panel builds nothing");
}

#[test]
fn a_radius_on_an_ellipse_is_not_offered_and_is_refused() {
    let mut a = open();
    select(&mut a, "oval");
    assert_eq!(binds(&a, "round"), [opt(BindField::Opacity, false)]);
    a.dispatch(Action::BindProperty {
        id: "oval".into(),
        field: BindField::Radius,
        variable: Some("round".into()),
    });
    assert_eq!(refused(&mut a), ["not_bindable"]);
}

#[test]
fn binding_the_variable_a_field_already_holds_is_no_change() {
    let mut a = open();
    a.dispatch(Action::BindProperty {
        id: "card".into(),
        field: BindField::Fill,
        variable: Some("bg".into()),
    });
    assert_eq!(a.state().history.undo_steps, 0);
    assert!(a.take_events().is_empty());
}

#[test]
fn the_search_text_is_cut_at_200_characters() {
    let mut a = open();
    a.dispatch(Action::SetVariablesQuery {
        query: "é".repeat(500),
    });
    assert_eq!(a.state().variables.query.chars().count(), 200);
}

#[test]
fn a_collection_whose_matches_are_all_beyond_the_cap_is_still_listed_with_its_count() {
    let mut d = file();
    for i in 0..250 {
        let c = cmd(
            json!({"type": "addVariable", "variable": {"id": format!("n{i}"), "collection": "t", "name": format!("number/n{i}"),
            "type": "number", "values": {"light": i, "dark": i}}}),
        );
        meldframe_core::apply_doc(&mut d, &c, &Default::default()).unwrap();
    }
    for c in [
        cmd(
            json!({"type": "addCollection", "collection": {"id": "u", "name": "spacing", "modes": [{"id": "m", "name": "m"}]}}),
        ),
        cmd(
            json!({"type": "addVariable", "variable": {"id": "gap", "collection": "u", "name": "space/gap", "type": "number",
            "values": {"m": 8}}}),
        ),
    ] {
        meldframe_core::apply_doc(&mut d, &c, &Default::default()).unwrap();
    }
    let mut a = App::with_doc("user:a", "t", 1, d, 0).unwrap();
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: true,
    });
    let v = &a.state().variables;
    assert_eq!((v.total, v.matched), (253, 253));
    let t = &v.collections[0];
    assert_eq!((t.matched, t.variables.len()), (252, 200));
    let u = &v.collections[1];
    assert_eq!((u.id.as_str(), u.matched, u.variables.len()), ("u", 1, 0));
}

fn bound_file() -> App {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "box", "type": "rect", "pos": "a0", "radius": {"var": "round"}, "opacity": {"var": "alpha"}},
        {"id": "label", "type": "text", "pos": "a1", "fill": {"var": "bg"}, "opacity": {"var": "alpha"}},
    ]))
    .unwrap();
    let mut d = Doc::of_nodes(nodes);
    for c in [
        appearance(),
        bg(),
        round(),
        cmd(
            json!({"type": "addVariable", "variable": {"id": "alpha", "collection": "t", "name": "number/alpha", "type": "number",
            "values": {"light": 0.5, "dark": 0.25}}}),
        ),
    ] {
        meldframe_core::apply_doc(&mut d, &c, &Default::default()).unwrap();
    }
    let mut a = App::with_doc("user:a", "t", 1, d, 0).unwrap();
    a.dispatch(Action::SetPanel {
        panel: Panel::Variables,
        open: true,
    });
    a.take_events();
    a
}

#[test]
fn unbinding_radius_opacity_and_a_text_node_keeps_the_painted_value() {
    let mut a = bound_file();
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    for (id, field) in [
        ("box", BindField::Radius),
        ("box", BindField::Opacity),
        ("label", BindField::Fill),
        ("label", BindField::Opacity),
    ] {
        a.dispatch(Action::BindProperty {
            id: id.into(),
            field,
            variable: None,
        });
    }
    assert!(refused(&mut a).is_empty());
    let n =
        |id: &str| serde_json::to_value(a.nodes().iter().find(|n| n.id == id).unwrap()).unwrap();
    assert_eq!(n("box")["radius"], json!(12.0));
    assert_eq!(n("box")["opacity"], json!(0.25));
    assert_eq!(n("label")["fill"], json!("#000000"));
    assert_eq!(n("label")["opacity"], json!(0.25));
    assert_eq!(row(&a, "alpha").used_by, 0);
    // The mode no longer reaches them.
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: None,
    });
    assert_eq!(a.scene()[0].radius, 12.0);
}

#[test]
fn a_variable_or_a_collection_removed_by_another_actor_leaves_no_stale_row() {
    let mut a = open();
    select(&mut a, "plain");
    a.dispatch(Action::Start { now_ms: 0 });
    a.dispatch(Action::Connected { now_ms: 0 });
    let remote = |seq: u64, command| meldframe_app::Action::Frame {
        now_ms: seq,
        frame: meldframe_app::Frame::Event {
            seq,
            actor: "user:b".into(),
            key: None,
            command,
        },
    };
    a.dispatch(remote(
        1,
        cmd(json!({"type": "removeVariable", "id": "round"})),
    ));
    let v = &a.state().variables;
    assert_eq!((v.total, v.matched), (1, 1));
    assert!(v.collections[0].variables.iter().all(|r| r.id != "round"));
    // A bind button for the gone variable is a refusal, not a panic.
    a.dispatch(Action::BindProperty {
        id: "plain".into(),
        field: BindField::Radius,
        variable: Some("round".into()),
    });
    assert_eq!(refused(&mut a), ["E_VARIABLE_NOT_FOUND"]);
    // The picked mode goes back to the default when the collection goes. The other actor lets the fill of the card
    // go first, as the server demands for a variable that is in use.
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    a.dispatch(remote(
        2,
        cmd(json!({"type": "updateNode", "id": "card", "patch": {"fill": "#123456"}})),
    ));
    a.dispatch(remote(
        3,
        cmd(json!({"type": "removeVariable", "id": "bg"})),
    ));
    a.dispatch(remote(
        4,
        cmd(json!({"type": "removeCollection", "id": "t"})),
    ));
    let v = &a.state().variables;
    assert_eq!((v.total, v.matched, v.collections.len()), (0, 0, 0));
    assert!(a.state().modes.is_empty());
    assert_eq!(a.state().selection, ["plain"]);
}

/// The server confirms my pending op `n` (the oldest) as event `seq`.
fn confirm(a: &mut App, seq: u64, command: meldframe_protocol::Command) {
    let key = a.state().pending[0].clone();
    a.dispatch(Action::Frame {
        now_ms: seq,
        frame: meldframe_app::Frame::Event {
            seq,
            actor: "user:a".into(),
            key: Some(key),
            command,
        },
    });
}

fn remote_set(a: &mut App, seq: u64, mode: &str, v: &str) {
    a.dispatch(Action::Frame {
        now_ms: seq,
        frame: meldframe_app::Frame::Event {
            seq,
            actor: "user:b".into(),
            key: None,
            command: common::set_bg(mode, v),
        },
    });
}

fn live() -> App {
    let mut a = open();
    a.dispatch(Action::Start { now_ms: 0 });
    a.dispatch(Action::Connected { now_ms: 0 });
    a
}

#[test]
fn undoing_my_value_after_another_actor_wrote_another_mode_still_works() {
    let mut a = live();
    a.dispatch(set("bg", "light", "#eeeeee"));
    confirm(&mut a, 1, common::set_bg("light", "#eeeeee"));
    remote_set(&mut a, 2, "dark", "#101010");
    assert_eq!(a.state().history.undo_steps, 1);
    a.dispatch(Action::Undo {});
    assert_eq!(
        texts(row(&a, "bg")),
        [("light", "#ffffff"), ("dark", "#101010")]
    );
}

#[test]
fn another_actor_writing_the_same_mode_after_me_drops_my_step() {
    let mut a = live();
    a.dispatch(set("bg", "light", "#dddddd"));
    confirm(&mut a, 1, common::set_bg("light", "#dddddd"));
    remote_set(&mut a, 2, "light", "#202020");
    assert_eq!(a.state().history.undo_steps, 0);
    assert_eq!(texts(row(&a, "bg"))[0], ("light", "#202020"));
}
