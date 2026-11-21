//! The canvas can read laid-out boxes: the app layer holds a host-injected text measurer and a host flag
//! (`layout_on_canvas`, off by default, not part of `AppState` or the file). With the flag off the scene is what it
//! always was; with it on, it is `resolve_laid_out` with the app's measurer.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{App, Effect, TextMeasurer};
use meldframe_protocol::Node;
use meldframe_render::{ModeChoice, PaintedBox, resolve_with};
use serde_json::{Value, json};

/// A column frame with padding and gap, a fixed child, a child that fills the width and a hug text. Every stored
/// offset is 0, as an agent that left them out would write them.
fn nodes() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "f", "type": "frame", "w": 200, "h": 300, "layout": "column", "padding": 10, "gap": 5},
        {"id": "a", "type": "rect", "parentId": "f", "pos": "a", "w": 50, "h": 20},
        {"id": "b", "type": "rect", "parentId": "f", "pos": "b", "w": 1, "h": 30, "sw": "fill"},
        {"id": "t", "type": "text", "parentId": "f", "pos": "c", "text": "hello", "fontSize": 16,
         "sw": "hug", "sh": "hug"},
    ]))
    .unwrap()
}

fn app() -> App {
    App::new("user:me", "c0", 1, nodes(), 0).unwrap()
}

fn geometry(app: &App, id: &str) -> (f64, f64, f64, f64) {
    let scene = app.scene();
    let b = scene.iter().find(|b| b.id == id).expect("a box");
    (b.x, b.y, b.w, b.h)
}

/// Every text measures to the same size, so a test sees whose answer the layout used.
struct Fixed(f64, f64);

impl TextMeasurer for Fixed {
    fn measure(&self, _: &Node, _: Option<f64>) -> (f64, f64) {
        (self.0, self.1)
    }
}

fn is_render_everything(fx: &[Effect]) -> bool {
    matches!(fx, [Effect::Render { nodes }] if nodes.is_empty())
}

#[test]
fn the_flag_is_off_and_the_scene_is_the_stored_boxes_to_the_bit() {
    let a = app();
    assert!(!a.layout_on_canvas());
    let stored: Vec<PaintedBox> = resolve_with(a.nodes(), a.vars(), &ModeChoice::default());
    assert_eq!(a.scene(), stored);
    assert_eq!(
        geometry(&a, "b"),
        (0.0, 0.0, 1.0, 30.0),
        "the stored box, not the fill"
    );
}

#[test]
fn with_the_flag_on_a_column_with_padding_and_gap_is_laid_out() {
    let mut a = app();
    a.set_layout_on_canvas(true);
    assert_eq!(geometry(&a, "f"), (0.0, 0.0, 200.0, 300.0));
    assert_eq!(geometry(&a, "a"), (10.0, 10.0, 50.0, 20.0));
    // Fill takes the width inside the padding: 200 - 2 * 10.
    assert_eq!(geometry(&a, "b"), (10.0, 35.0, 180.0, 30.0));
    // The stub measures "hello" at 16 px as 40 x 20.
    assert_eq!(geometry(&a, "t"), (10.0, 70.0, 40.0, 20.0));
}

#[test]
fn the_measurer_the_host_injects_is_the_one_the_layout_asks() {
    let mut a = app();
    a.set_layout_on_canvas(true);
    a.set_measurer(Box::new(Fixed(77.0, 33.0)));
    assert_eq!(geometry(&a, "t"), (10.0, 70.0, 77.0, 33.0));
    // Nothing but the hug text depends on it.
    assert_eq!(geometry(&a, "b"), (10.0, 35.0, 180.0, 30.0));
}

#[test]
fn a_measurer_changes_nothing_while_the_flag_is_off() {
    let mut a = app();
    let before = a.scene();
    let fx = a.set_measurer(Box::new(Fixed(77.0, 33.0)));
    assert!(fx.is_empty(), "no redraw: the canvas reads stored boxes");
    assert_eq!(a.scene(), before);
}

#[test]
fn a_new_measurer_under_the_flag_asks_for_a_redraw_of_everything() {
    let mut a = app();
    a.set_layout_on_canvas(true);
    assert!(is_render_everything(
        &a.set_measurer(Box::new(Fixed(1.0, 2.0)))
    ));
}

#[test]
fn toggling_the_flag_changes_the_next_scene_and_asks_for_a_redraw_only_when_it_changed() {
    let mut a = app();
    let off = a.scene();
    assert!(a.set_layout_on_canvas(false).is_empty(), "already off");
    assert!(is_render_everything(&a.set_layout_on_canvas(true)));
    assert!(a.layout_on_canvas());
    let on = a.scene();
    assert_ne!(on, off);
    assert!(a.set_layout_on_canvas(true).is_empty(), "already on");
    assert_eq!(a.scene(), on);
    assert!(is_render_everything(&a.set_layout_on_canvas(false)));
    assert_eq!(a.scene(), off, "off again: the stored boxes, to the bit");
}

#[test]
fn the_flag_and_the_measurer_are_host_configuration_not_state_and_not_the_file() {
    let mut a = app();
    let state = serde_json::to_value(a.state()).unwrap();
    let doc = serde_json::to_value(a.nodes()).unwrap();
    a.set_layout_on_canvas(true);
    a.set_measurer(Box::new(Fixed(77.0, 33.0)));
    assert_eq!(serde_json::to_value(a.state()).unwrap(), state);
    assert_eq!(serde_json::to_value(a.nodes()).unwrap(), doc);
    assert!(
        a.take_events().is_empty(),
        "the UI is told nothing: the host asked"
    );
    let text: Value = serde_json::to_value(a.state()).unwrap();
    assert!(!text.to_string().contains("layoutOnCanvas"));
}

#[test]
fn the_export_dialog_keeps_the_measurer_it_had() {
    // Export output does not change with the canvas flag or the injected measurer (the dialog still measures with
    // the stub; moving it to the app's measurer is a later, separate change).
    let html = |a: &mut App| {
        use meldframe_app::Action;
        a.dispatch(Action::Select {
            ids: vec!["f".into()],
        });
        a.dispatch(Action::OpenExport {});
        a.state()
            .export
            .dialog
            .as_ref()
            .expect("a dialog")
            .html
            .clone()
    };
    let plain = html(&mut app());
    let mut a = app();
    a.set_layout_on_canvas(true);
    a.set_measurer(Box::new(Fixed(77.0, 33.0)));
    assert_eq!(html(&mut a), plain);
}

/// A desktop shell keeps the `App` behind a mutex, so on a native target the app and the measurer it holds are
/// `Send`. (Under WASM there is one thread, and a measurer that calls into JavaScript cannot be `Send`.)
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_app_can_move_between_threads_on_a_native_host() {
    fn is_send<T: Send>() {}
    is_send::<App>();
    // The measurer a native host hands over must be `Send` too: this line fails to compile otherwise.
    let _ = app().set_measurer(Box::new(Fixed(1.0, 1.0)));
    let mut a = app();
    std::thread::spawn(move || a.set_layout_on_canvas(true))
        .join()
        .unwrap();
}
