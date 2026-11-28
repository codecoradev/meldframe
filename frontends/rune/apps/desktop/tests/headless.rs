//! The headless tests of the desktop shell (E8.2.2): the real `Runtime` of the app with no window, no GPU and no server. A
//! click, the Move and Undo buttons, the status and the cached canvas shape are driven through Rune's own input and read back
//! from the element tree and from the app layer. The element tree is stored as a golden and is byte stable across two runs.
//!
//! The PNG golden needs a GPU adapter, so it has its own test in `render.rs`.

use meldframe_app::{Action, App, Connection, Direction, Frame};
use meldframe_desktop::{State, WINDOW_SIZE, spec};
use meldframe_protocol::{Command, Node, NodePatch};
use meldframe_ui_rune::BAR_HEIGHT;
use rune::shape::Op;
use rune::{Env, Runtime};
use serde_json::{Value, json};

mod common;
use common::fixture_nodes;

fn fixture() -> State {
    State::with_nodes(fixture_nodes(), true)
}

fn runtime(init: fn() -> State) -> Runtime<State> {
    // Text shapes with the bundled face only, so that the metrics do not depend on the machine.
    rune::text::set_system_fonts(false);
    let mut rt = rune::App::new(spec(init)).headless_runtime(WINDOW_SIZE, Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn click(rt: &mut Runtime<State>, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

fn click_key(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no element keyed {key}"));
    click(rt, x, y);
}

fn node<'a>(app: &'a App, id: &str) -> &'a Node {
    app.nodes()
        .iter()
        .find(|n| n.id == id)
        .expect("the node exists")
}

fn status(rt: &Runtime<State>) -> String {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("status"))
        .and_then(|e| e.text)
        .expect("the status shows text")
}

#[test]
fn a_click_on_the_canvas_selects_what_the_app_layer_finds_under_it() {
    let mut rt = runtime(fixture);
    // `r1` is a rectangle at (110, 110) of the world, 50 by 40; the canvas starts below the bar, at the left edge.
    click(&mut rt, 120.0, BAR_HEIGHT + 120.0);
    assert_eq!(rt.state.app().state().selection, ["r1"]);
    assert_eq!(
        rt.state.app().hit(120.0, 120.0).as_deref(),
        Some("r1"),
        "the app layer's own answer for the same point"
    );

    // A point over the card and not over the rectangle selects the card; one over nothing clears the selection.
    click(&mut rt, 300.0, BAR_HEIGHT + 250.0);
    assert_eq!(rt.state.app().state().selection, ["f1"]);
    click(&mut rt, 880.0, BAR_HEIGHT + 590.0);
    assert!(rt.state.app().state().selection.is_empty());
}

#[test]
fn move_and_undo_send_the_actions_the_app_layer_defines() {
    let mut rt = runtime(fixture);
    // With nothing selected Move does nothing: the button is disabled.
    let before = rt.state.outbox().len();
    assert!(!rt.state.app().can_nudge(Direction::Right, true));
    click_key(&mut rt, "move");
    assert_eq!(rt.state.outbox().len(), before);

    click(&mut rt, 120.0, BAR_HEIGHT + 120.0);
    let start = node(rt.state.app(), "r1").x;

    // The same action on a twin app says what the button must have done: the selection moves by a coarse step to the right,
    // which is 10 units and is the app layer's rule.
    let mut twin = App::new("user:twin", "twin", 1, fixture_nodes(), 0).expect("loads");
    twin.dispatch(Action::Select {
        ids: vec!["r1".into()],
    });
    twin.dispatch(Action::Nudge {
        direction: Direction::Right,
        coarse: true,
    });
    let moved = node(&twin, "r1").x;
    assert_eq!(moved, start + 10.0);

    click_key(&mut rt, "move");
    assert_eq!(node(rt.state.app(), "r1").x, moved);
    assert!(
        rt.state.outbox().len() > before,
        "the change is an op the transport has to send"
    );

    click_key(&mut rt, "undo");
    assert_eq!(node(rt.state.app(), "r1").x, start);
    assert!(!rt.state.app().state().history.can_undo);
}

#[test]
fn the_status_shows_the_connection_state_of_the_app_layer() {
    let mut rt = runtime(State::offline);
    assert_eq!(rt.state.app().state().connection, Connection::Connecting);
    assert_eq!(status(&rt), "Connecting");

    rt.update(|s, _| s.act(Action::Connected { now_ms: 0 }));
    rt.frame(0.0);
    assert_eq!(rt.state.app().state().connection, Connection::Live);
    assert_eq!(status(&rt), "Connected");

    rt.update(|s, _| s.act(Action::Disconnected {}));
    rt.frame(0.0);
    let shown = rt.state.app().state().connection;
    assert_ne!(shown, Connection::Live);
    assert!(
        !status(&rt).is_empty() && status(&rt) != "Connected",
        "{shown:?} is shown as {}",
        status(&rt)
    );
}

#[test]
fn the_canvas_shape_is_rebuilt_only_when_the_scene_epoch_moves() {
    let mut rt = runtime(fixture);
    assert_eq!(rt.state.shape().builds(), 1);
    let first = rt.state.shape().epoch().expect("built");
    let resolves = rt.state.app().scene_resolves();

    // Selection, hover and a view that runs again for them leave the epoch where it is.
    click(&mut rt, 120.0, BAR_HEIGHT + 120.0);
    rt.pointer_move(130.0, BAR_HEIGHT + 130.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(
        rt.state.shape().builds(),
        1,
        "no rebuild for a selection or a hover"
    );
    assert_eq!(
        rt.state.app().scene_resolves(),
        resolves,
        "and no layout work"
    );

    // An edit moves it, and the shape is built once more.
    click_key(&mut rt, "move");
    let second = rt.state.shape().epoch().expect("built");
    assert!(second > first);
    assert_eq!(rt.state.shape().builds(), 2);
    assert_eq!(rt.state.app().scene_epoch(), second);
}

/// A column with two children in the flow and a free rectangle, the layout on the canvas on.
fn laid_out() -> State {
    let nodes = serde_json::from_value(json!([
        {"id": "col", "type": "frame", "x": 0, "y": 0, "w": 200, "h": 300, "layout": "column"},
        {"id": "c1", "type": "rect", "parentId": "col", "w": 50, "h": 20},
        {"id": "c2", "type": "rect", "parentId": "col", "x": 0, "w": 50, "h": 20},
        {"id": "free", "type": "rect", "x": 400, "y": 0, "w": 50, "h": 20}
    ]))
    .expect("parses");
    let mut state = State::with_nodes(nodes, true);
    state.set_layout_on_canvas(true);
    state
}

fn select(rt: &mut Runtime<State>, ids: &[&str]) {
    let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
    rt.update(|s, _| s.act(Action::Select { ids }));
    rt.frame(0.0);
}

fn move_is_on(rt: &Runtime<State>) -> bool {
    rt.elements()
        .iter()
        .any(|e| e.key.as_deref() == Some("move") && e.focusable)
}

#[test]
fn move_is_off_for_the_press_it_sends_when_that_press_moves_nothing_or_is_refused() {
    let mut rt = runtime(laid_out);
    // A child of an auto layout frame: a coarse press leaves it where it is, so the button that sends one does nothing.
    select(&mut rt, &["c1"]);
    assert!(!rt.state.app().can_nudge(Direction::Right, true));
    assert!(
        !move_is_on(&rt),
        "a flow child: the coarse press moves nothing"
    );
    let (outbox, steps) = (
        rt.state.outbox().len(),
        rt.state.app().state().history.undo_steps,
    );
    click_key(&mut rt, "move");
    assert_eq!(rt.state.outbox().len(), outbox);
    assert_eq!(rt.state.app().state().history.undo_steps, steps);

    // A free node moves, and is on.
    select(&mut rt, &["free"]);
    assert!(move_is_on(&rt));
    click_key(&mut rt, "move");
    assert_eq!(node(rt.state.app(), "free").x, 410.0);

    // At the limit of a coordinate the press is refused whole: the button is off though a press to the left would be taken.
    rt.update(|s, _| {
        s.act(Action::Move {
            id: "free".into(),
            x: 1.0e6,
            y: 0.0,
        })
    });
    rt.frame(0.0);
    assert!(rt.state.app().can_nudge(Direction::Left, true));
    assert!(!rt.state.app().can_nudge(Direction::Right, true));
    assert!(!move_is_on(&rt), "past the limit: refused, so off");
}

/// The corners `[x0, y0, x1, y1]` of what one fill of the shape covers.
fn bounds(op: &Op) -> Option<[f32; 4]> {
    let points: Vec<[f32; 2]> = match op {
        Op::Fill { points, .. } => points.clone(),
        Op::FillCompound { contours, .. } => contours.concat(),
        _ => return None,
    };
    let fold = |pick: fn(&[f32; 2]) -> f32, lo: bool| {
        points
            .iter()
            .map(pick)
            .fold(if lo { f32::MAX } else { f32::MIN }, |a, v| {
                if lo { a.min(v) } else { a.max(v) }
            })
    };
    Some([
        fold(|p| p[0], true),
        fold(|p| p[1], true),
        fold(|p| p[0], false),
        fold(|p| p[1], false),
    ])
}

#[test]
fn the_shape_paints_a_child_where_the_hit_rule_finds_it() {
    let state = fixture();
    let shape = state
        .app()
        .with_scene(|_, boxes| canvas_ui_rune::paint(boxes));
    let fills: Vec<[f32; 4]> = shape.ops.iter().filter_map(bounds).collect();
    // `r1` is at (10, 10) inside `f1`, which is at (100, 100): the world rectangle is (110, 110) to (160, 150), and that is
    // where the app layer finds it.
    assert_eq!(state.app().hit(135.0, 130.0).as_deref(), Some("r1"));
    assert!(
        fills.contains(&[110.0, 110.0, 160.0, 150.0]),
        "no fill covers the world rectangle of r1: {fills:?}"
    );
    assert_eq!(fills.len(), 4, "one fill for each of the four boxes");
}

#[test]
fn move_is_enabled_only_when_the_app_layer_would_take_it() {
    let mut rt = runtime(fixture);
    click(&mut rt, 120.0, BAR_HEIGHT + 120.0);
    let enabled = |rt: &Runtime<State>, key: &str| {
        rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some(key) && e.focusable)
    };
    assert!(
        enabled(&rt, "move"),
        "a selection on a live connection: the button works"
    );
    assert!(
        rt.state.app().can_nudge(Direction::Right, true),
        "and the app layer says so, for the press the button sends"
    );
    click_key(&mut rt, "move");
    assert!(enabled(&rt, "undo"), "and so does Undo after a change");

    // A frame the file cannot take halts the connection: the app layer then drops every change.
    rt.update(|s, _| {
        s.act(Action::Frame {
            now_ms: 0,
            frame: Frame::Event {
                seq: 1,
                actor: "agent:x".into(),
                key: Some("b1:0".into()),
                command: Command::UpdateNode {
                    id: "ghost".into(),
                    patch: NodePatch::default(),
                },
            },
        });
    });
    rt.frame(0.0);
    assert_eq!(rt.state.app().state().connection, Connection::Halted);
    assert!(
        !rt.state.app().state().selection.is_empty(),
        "the selection is still there"
    );
    assert!(!enabled(&rt, "move"), "halted: Move is off");
    assert!(
        !enabled(&rt, "undo"),
        "halted: Undo is off, though there is a step to undo"
    );
    assert!(rt.state.app().state().history.can_undo);

    // The buttons agree with the app layer: the same actions do nothing while halted.
    let before = node(rt.state.app(), "r1").x;
    rt.update(|s, _| {
        s.act(Action::Nudge {
            direction: Direction::Right,
            coarse: true,
        });
        s.act(Action::Undo {});
    });
    assert_eq!(node(rt.state.app(), "r1").x, before);
}

/// The element tree as a stable JSON text: what a person or a tool reads to see what the window holds. Floats are rounded to
/// hundredths and colours are hex, so the text does not depend on how the floats print.
fn tree(rt: &Runtime<State>) -> String {
    let round = |v: f32| (f64::from(v) * 100.0).round() / 100.0;
    let elements: Vec<Value> = rt
        .elements()
        .into_iter()
        .map(|e| {
            json!({
                "path": e.path,
                "key": e.key,
                "role": e.role,
                "label": e.label,
                "text": e.text,
                "rect": e.rect.map(round),
                "fill": e.fill.hex(),
                "radius": round(e.radius),
                "opacity": round(e.opacity),
                "focusable": e.focusable,
                "parent": e.parent,
            })
        })
        .collect();
    let mut text = serde_json::to_string_pretty(
        &json!({ "window": [WINDOW_SIZE.0, WINDOW_SIZE.1], "elements": elements }),
    )
    .expect("json");
    text.push('\n');
    text
}

#[test]
fn the_element_tree_is_a_golden_and_is_stable_across_two_runs() {
    let first = tree(&runtime(fixture));
    let second = tree(&runtime(fixture));
    assert_eq!(first, second, "two runs give the same bytes");
    common::golden("tree.json", first.as_bytes());
}

#[test]
fn a_tick_or_a_heartbeat_is_no_change_for_the_view_and_a_selection_is() {
    // The network half tells the app the time every second: the window must not draw again for it.
    let mut s = fixture();
    assert!(!s.apply(Action::Tick { now_ms: 1 }));
    assert!(!s.apply(Action::Frame {
        now_ms: 2,
        frame: Frame::Heartbeat {}
    }));
    assert!(s.apply(Action::Select {
        ids: vec!["r1".into()]
    }));
    assert!(s.apply(Action::Disconnected {}), "the connection changed");
}
