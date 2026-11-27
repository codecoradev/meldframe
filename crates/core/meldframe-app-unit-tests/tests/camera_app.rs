//! `Action::Camera` through the app (E8.1.4, part 1), with no front end: pan, zoom to a pointer, fit all, zoom to the selection and
//! 100%, read from state only. The rules are in `camera_rules.rs`; the hit test (`App::hit`) is still the only rule for what is
//! under a point, and the camera never reaches the document or the history.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{
    Action, App, Camera, CameraOp, Connection, Event, Point, Viewport, ZoomCommand,
};
use meldframe_app_model::camera::{ZOOM_MAX, to_screen, to_world};
use meldframe_protocol::{Command, Node, NodePatch};
use serde_json::{Value, json};

fn file() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "a", "type": "frame", "pos": "a0", "w": 200, "h": 100},
        {"id": "b", "type": "frame", "pos": "a1", "x": 800, "y": 400, "w": 200, "h": 200},
        {"id": "c", "type": "rect", "parentId": "a", "pos": "a0", "x": 10, "y": 10, "w": 50, "h": 50},
    ]))
    .expect("a valid value")
}

fn editor() -> App {
    let mut app = App::new("user:me", "c0", 1, file(), 0).expect("a valid value");
    app.take_events();
    let fx = app.dispatch(camera(CameraOp::SetViewport {
        w: 1000.0,
        h: 600.0,
    }));
    assert!(fx.is_empty());
    app.take_events();
    app
}

fn camera(op: CameraOp) -> Action {
    Action::Camera { op }
}

fn zoom(command: ZoomCommand) -> Action {
    camera(CameraOp::Zoom { command })
}

fn at(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} is not {b}");
}

fn cam(app: &App) -> Camera {
    app.state().camera
}

#[test]
fn the_camera_starts_at_home_and_the_viewport_is_what_the_front_end_reported() {
    let app = App::new("user:me", "c0", 1, file(), 0).expect("a valid value");
    assert_eq!(
        cam(&app),
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 1.0
        }
    );
    assert_eq!(app.state().viewport, Viewport { w: 0.0, h: 0.0 });
    assert_eq!(
        editor().state().viewport,
        Viewport {
            w: 1000.0,
            h: 600.0
        }
    );
}

#[test]
fn pan_zoom_to_the_pointer_fit_zoom_to_selection_and_100_percent_read_from_state_only() {
    let mut app = editor();
    // Pan: the world moves with the pointer.
    app.dispatch(camera(CameraOp::Pan {
        dx: 30.0,
        dy: -20.0,
    }));
    assert_eq!(
        cam(&app),
        Camera {
            x: 30.0,
            y: -20.0,
            zoom: 1.0
        }
    );
    assert_eq!(app.take_events(), vec![Event::CameraChanged {}]);

    // Zoom to the pointer: the world point under it stays under it.
    let pointer = at(400.0, 250.0);
    let under = to_world(&cam(&app), pointer);
    app.dispatch(camera(CameraOp::ZoomAt {
        factor: 2.0,
        at: pointer,
    }));
    assert_eq!(cam(&app).zoom, 2.0);
    let (after, w) = (to_screen(&cam(&app), under), pointer);
    close(after.x, w.x);
    close(after.y, w.y);

    // Fit all: the two frames, 1000 by 600, centred with 48 px around (the child is inside a frame and adds nothing).
    app.dispatch(zoom(ZoomCommand::Fit));
    let c = cam(&app);
    close(c.zoom, 0.84);
    close(c.x, 80.0);
    close(c.y, 48.0);

    // Zoom to the selection: the child alone, 50 by 50, at (10, 10).
    app.dispatch(Action::Select { ids: ids(&["c"]) });
    app.take_events();
    app.dispatch(zoom(ZoomCommand::FitSelection));
    let c = cam(&app);
    close(c.zoom, 504.0 / 50.0);
    close(c.x, 500.0 - 35.0 * c.zoom);
    close(c.y, 300.0 - 35.0 * c.zoom);
    assert_eq!(app.take_events(), vec![Event::CameraChanged {}]);

    // With nothing selected it shows all the content.
    app.dispatch(Action::ClearSelection {});
    app.dispatch(zoom(ZoomCommand::FitSelection));
    close(cam(&app).zoom, 0.84);

    // 100%: the centre of the view stays where it is.
    let centre = to_world(&cam(&app), at(500.0, 300.0));
    app.dispatch(zoom(ZoomCommand::Reset));
    assert_eq!(cam(&app).zoom, 1.0);
    let s = to_screen(&cam(&app), centre);
    close(s.x, 500.0);
    close(s.y, 300.0);
}

#[test]
fn zoom_in_and_out_take_a_step_and_stop_at_the_limits() {
    let mut app = editor();
    app.dispatch(zoom(ZoomCommand::In));
    close(cam(&app).zoom, 1.25);
    app.dispatch(zoom(ZoomCommand::Out));
    close(cam(&app).zoom, 1.0);
    app.dispatch(camera(CameraOp::ZoomAt {
        factor: 1e9,
        at: at(0.0, 0.0),
    }));
    assert_eq!(cam(&app).zoom, ZOOM_MAX);
    app.take_events();
    app.dispatch(zoom(ZoomCommand::In));
    assert!(app.take_events().is_empty(), "no change, no event");
}

#[test]
fn a_wheel_and_a_pinch_come_in_as_json_and_move_the_camera() {
    let mut app = editor();
    let pan: Action = serde_json::from_value(json!({"type": "camera", "op": {"type": "wheel",
        "wheel": {"dx": 0, "dy": 30}, "at": {"x": 10, "y": 10}}}))
    .expect("a valid value");
    app.dispatch(pan);
    assert_eq!(
        cam(&app),
        Camera {
            x: 0.0,
            y: -30.0,
            zoom: 1.0
        }
    );
    let pinch: Action = serde_json::from_value(json!({"type": "camera", "op": {"type": "pinch",
        "from": [{"x": 100, "y": 100}, {"x": 200, "y": 100}],
        "to": [{"x": 100, "y": 100}, {"x": 300, "y": 100}]}}))
    .expect("a valid value");
    app.dispatch(pinch);
    close(cam(&app).zoom, 2.0);
    let ctrl: Action = serde_json::from_value(json!({"type": "camera", "op": {"type": "wheel",
        "wheel": {"dx": 0, "dy": -10, "mods": {"ctrl": true}}, "at": {"x": 10, "y": 10}}}))
    .expect("a valid value");
    app.dispatch(ctrl);
    assert!(cam(&app).zoom > 2.0);
}

#[test]
fn a_number_that_is_not_a_number_is_refused_and_nothing_changes() {
    let mut app = editor();
    app.dispatch(camera(CameraOp::Pan { dx: 5.0, dy: 5.0 }));
    app.take_events();
    let before = (cam(&app), app.state().viewport);
    for op in [
        CameraOp::Pan {
            dx: f64::NAN,
            dy: 0.0,
        },
        CameraOp::Pan {
            dx: f64::INFINITY,
            dy: 0.0,
        },
        CameraOp::ZoomAt {
            factor: 0.0,
            at: at(1.0, 1.0),
        },
        CameraOp::ZoomAt {
            factor: -2.0,
            at: at(1.0, 1.0),
        },
        CameraOp::ZoomAt {
            factor: f64::NAN,
            at: at(1.0, 1.0),
        },
        CameraOp::SetViewport { w: -1.0, h: 10.0 },
        CameraOp::SetViewport {
            w: f64::NAN,
            h: 10.0,
        },
        CameraOp::Pinch {
            from: [at(f64::NAN, 0.0), at(1.0, 1.0)],
            to: [at(0.0, 0.0), at(2.0, 2.0)],
        },
    ] {
        app.dispatch(camera(op.clone()));
        let events = app.take_events();
        assert_eq!(
            events,
            vec![Event::Refused {
                code: "invalid".into()
            }],
            "{op:?}"
        );
        assert_eq!((cam(&app), app.state().viewport), before, "{op:?}");
    }
    // A sum that overflows is refused too: the camera stays finite.
    app.dispatch(camera(CameraOp::Pan {
        dx: f64::MAX,
        dy: 0.0,
    }));
    app.take_events();
    app.dispatch(camera(CameraOp::Pan {
        dx: f64::MAX,
        dy: 0.0,
    }));
    assert_eq!(
        app.take_events(),
        vec![Event::Refused {
            code: "invalid".into()
        }]
    );
    assert!(cam(&app).x.is_finite());
}

#[test]
fn a_camera_change_touches_neither_the_document_nor_the_history_and_raises_one_event() {
    let mut app = editor();
    app.dispatch(Action::Move {
        id: "b".into(),
        x: 900.0,
        y: 400.0,
    });
    app.take_events();
    let (nodes, history) = (
        serde_json::to_value(app.nodes()).expect("a valid value"),
        app.state().history,
    );
    let epoch = app.scene_epoch();
    for a in [
        camera(CameraOp::Pan { dx: 1.0, dy: 1.0 }),
        zoom(ZoomCommand::Fit),
        zoom(ZoomCommand::Reset),
    ] {
        let fx = app.dispatch(a);
        assert!(fx.is_empty(), "no effect: {fx:?}");
    }
    assert_eq!(app.take_events(), vec![Event::CameraChanged {}; 3]);
    assert_eq!(
        serde_json::to_value(app.nodes()).expect("a valid value"),
        nodes
    );
    assert_eq!(app.state().history, history);
    assert_eq!(
        app.scene_epoch(),
        epoch,
        "a camera is no input of the scene"
    );
    // What is under a point is still the one rule of `App::hit`, in world space: the camera does not enter it.
    assert_eq!(app.hit(950.0, 450.0).as_deref(), Some("b"));
}

#[test]
fn a_halted_app_still_pans_and_zooms() {
    let mut l = Loopback::live();
    l.remote(
        "agent:a",
        "k",
        1,
        Command::UpdateNode {
            id: "ghost".into(),
            patch: NodePatch::default(),
        },
    );
    assert_eq!(l.app.state().connection, Connection::Halted);
    l.send(camera(CameraOp::Pan { dx: 7.0, dy: 0.0 }));
    assert_eq!(l.app.state().camera.x, 7.0);
}

#[test]
fn a_camera_action_is_a_view_action_like_the_other_state_that_never_reaches_the_file() {
    assert!(meldframe_app_rules::level::is_view(&zoom(ZoomCommand::Fit)));
}

#[test]
fn the_camera_is_part_of_the_state_json_and_a_state_without_it_still_reads() {
    let app = editor();
    let mut v = serde_json::to_value(app.state()).expect("a valid value");
    assert_eq!(v["camera"], json!({"x": 0.0, "y": 0.0, "zoom": 1.0}));
    assert_eq!(v["viewport"], json!({"w": 1000.0, "h": 600.0}));
    v.as_object_mut().expect("a valid value").remove("camera");
    v.as_object_mut().expect("a valid value").remove("viewport");
    let s: meldframe_app::AppState = serde_json::from_value(v).expect("a valid value");
    assert_eq!(s.camera, Camera::default());
}

// Fits on files whose nodes are inside frames that are not at the origin, with the stored boxes and the laid-out ones.

/// A frame at (500, 300), a frame in it at (20, 20) and a rectangle in that at (5, 5), and a rectangle `c` in the first at (10, 10).
fn nodes(list: Value) -> Vec<Node> {
    serde_json::from_value(list).expect("a valid value")
}

fn nested() -> Value {
    json!([
        {"id": "a", "type": "frame", "pos": "a0", "x": 500, "y": 300, "w": 200, "h": 100},
        {"id": "c", "type": "rect", "parentId": "a", "pos": "a0", "x": 10, "y": 10, "w": 50, "h": 50},
        {"id": "m", "type": "frame", "parentId": "a", "pos": "a1", "x": 20, "y": 20, "w": 100, "h": 60},
        {"id": "g", "type": "rect", "parentId": "m", "pos": "a0", "x": 5, "y": 5, "w": 10, "h": 10},
    ])
}

fn editor_of(list: Value, layout: bool) -> App {
    let mut app = App::new("user:me", "c0", 1, nodes(list), 0).expect("a valid value");
    app.set_layout_on_canvas(layout);
    app.dispatch(Action::Camera {
        op: CameraOp::SetViewport {
            w: 1000.0,
            h: 600.0,
        },
    });
    app.take_events();
    app
}

fn zoom_to(app: &mut App, command: ZoomCommand) -> Camera {
    app.dispatch(Action::Camera {
        op: CameraOp::Zoom { command },
    });
    app.state().camera
}

#[test]
fn zoom_to_selection_centres_a_node_that_is_inside_a_frame_off_the_origin_with_the_layout_on_and_off()
 {
    for layout in [false, true] {
        let mut app = editor_of(nested(), layout);
        app.dispatch(Action::Select { ids: ids(&["c"]) });
        let c = zoom_to(&mut app, ZoomCommand::FitSelection);
        // 50 by 50 at (510, 310): the height decides, 504 / 50, and the centre (535, 335) is the centre of the view
        close(c.zoom, 10.08);
        close(c.x, 500.0 - 535.0 * 10.08);
        close(c.y, 300.0 - 335.0 * 10.08);
        // the grandchild
        app.dispatch(Action::Select { ids: ids(&["g"]) });
        let c = zoom_to(&mut app, ZoomCommand::FitSelection);
        close(c.zoom, 50.4);
        close(c.x, 500.0 - 530.0 * 50.4);
        close(c.y, 300.0 - 330.0 * 50.4);
    }
}

#[test]
fn fit_all_shows_the_frame_off_the_origin_and_a_file_with_nothing_in_it_centres_the_origin() {
    for layout in [false, true] {
        let mut app = editor_of(nested(), layout);
        let c = zoom_to(&mut app, ZoomCommand::Fit);
        // 200 by 100 at (500, 300): the width decides, 904 / 200
        close(c.zoom, 4.52);
        close(c.x, 500.0 - 600.0 * 4.52);
        close(c.y, 300.0 - 350.0 * 4.52);
    }
    let mut empty = editor_of(json!([]), false);
    assert_eq!(
        zoom_to(&mut empty, ZoomCommand::Fit),
        Camera {
            x: 500.0,
            y: 300.0,
            zoom: 1.0
        }
    );
    assert_eq!(
        zoom_to(&mut empty, ZoomCommand::FitSelection),
        Camera {
            x: 500.0,
            y: 300.0,
            zoom: 1.0
        }
    );
    assert!(
        empty.take_events().len() == 1,
        "the second fit changes nothing: one event for the two"
    );
}

#[test]
fn a_fit_before_the_viewport_is_known_changes_nothing_and_says_nothing() {
    let mut app = App::new("user:me", "c0", 1, nodes(nested()), 0).expect("a valid value");
    app.take_events();
    assert_eq!(app.state().viewport, Viewport { w: 0.0, h: 0.0 });
    for command in [ZoomCommand::Fit, ZoomCommand::FitSelection] {
        zoom_to(&mut app, command);
        assert_eq!(app.state().camera, Camera::default());
        assert!(app.take_events().is_empty(), "{command:?}");
    }
}
