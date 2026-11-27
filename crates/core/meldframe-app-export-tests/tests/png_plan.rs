//! The PNG export of a frame (finding T13 of the thin UI audit, decision 43): the app layer decides the rectangle (from
//! `paint_bounds`: the outside border and the shadows count), the scale, the size in pixels, the file name and the refusal, and
//! hands the shell one effect with everything it needs to draw; what the shell reports comes back as `Wire` answers.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, Effect, Event, PngOp, PngSink};
use meldframe_app_model::RasterRect;
use meldframe_protocol::Node;
use serde_json::{Value, json};

fn nodes(list: Value) -> Vec<Node> {
    serde_json::from_value(list).unwrap()
}

/// An app whose file is the frames of `list`, with `id` selected.
fn app_with(list: Value, id: &str) -> App {
    let mut app = App::new("user:me", "c0", 1, nodes(list), 0).unwrap();
    app.dispatch(Action::Select {
        ids: vec![id.into()],
    });
    app.take_events();
    app
}

fn frame(name: &str, w: u32, h: u32) -> Value {
    json!([{"id": "f", "type": "frame", "name": name, "x": 100, "y": 50, "w": w, "h": h}])
}

fn png(app: &mut App, op: PngOp) -> Vec<Effect> {
    app.dispatch(Action::Png { op })
}

fn export(app: &mut App) -> Vec<Effect> {
    png(
        app,
        PngOp::Export {
            id: "f".into(),
            to: PngSink::Download,
        },
    )
}

fn plan(app: &App) -> Value {
    serde_json::to_value(app.state().export.png.target.clone()).unwrap()
}

fn effects_json(fx: &[Effect]) -> Vec<Value> {
    fx.iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect()
}

fn refusals(app: &mut App) -> Vec<String> {
    app.take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect()
}

#[test]
fn the_plan_of_the_selected_frame_has_the_rectangle_with_its_border_and_shadow_the_size_and_the_file()
 {
    let list = json!([{"id": "f", "type": "frame", "name": "Landing", "x": 100, "y": 50, "w": 200, "h": 100,
        "border": {"width": 4, "color": "#000000", "position": "outside"},
        "shadows": [{"x": 10, "y": 6, "blur": 8, "spread": 2, "color": "#000000"}]}]);
    let app = app_with(list, "f");
    // The border adds 4 on each side; the shadow reaches 0 left, 4 above (and the border's 4 is more), 20 right, 16 below.
    assert_eq!(
        plan(&app),
        json!({"type": "ready", "rect": {"x": 96.0, "y": 46.0, "w": 224.0, "h": 120.0}, "scale": 1.0,
               "width": 224, "height": 120, "file": "Landing.png"})
    );
    assert_eq!(app.state().export.png.scale, 1.0);
    assert!(app.state().export.png.dialog.is_none());
}

#[test]
fn no_frame_selected_means_no_plan() {
    let mut app = app_with(frame("A", 10, 10), "f");
    app.dispatch(Action::ClearSelection {});
    assert!(app.state().export.png.target.is_none());
}

#[test]
fn the_scale_changes_the_size_and_the_file_and_is_an_event() {
    let mut app = app_with(frame("Hero", 200, 100), "f");
    png(&mut app, PngOp::SetScale { scale: 2.0 });
    assert_eq!(app.take_events(), vec![Event::ExportChanged {}]);
    assert_eq!(plan(&app)["width"], 400);
    assert_eq!(plan(&app)["file"], "Hero@2x.png");
    png(&mut app, PngOp::SetScale { scale: 0.5 });
    assert_eq!(
        (plan(&app)["width"].clone(), plan(&app)["file"].clone()),
        (json!(100), json!("Hero@0.5x.png"))
    );
    // Each side is rounded up, so no edge is cut.
    png(&mut app, PngOp::SetScale { scale: 3.0 });
    assert_eq!(plan(&app)["height"], 300);
    let mut odd = app_with(frame("Odd", 101, 33), "f");
    png(&mut odd, PngOp::SetScale { scale: 0.5 });
    assert_eq!(
        (plan(&odd)["width"].clone(), plan(&odd)["height"].clone()),
        (json!(51), json!(17))
    );
}

#[test]
fn a_scale_outside_half_to_four_is_refused_before_anything_is_drawn() {
    let mut app = app_with(frame("Hero", 200, 100), "f");
    // Outside 0.5 to 4, and between the steps: the section offers 0.5, 1, 2, 3 and 4.
    for scale in [
        0.4,
        0.0,
        -1.0,
        4.5,
        1000.0,
        1.37,
        2.5,
        f64::NAN,
        f64::INFINITY,
    ] {
        let fx = png(&mut app, PngOp::SetScale { scale });
        assert!(fx.is_empty(), "{scale}");
        assert_eq!(refusals(&mut app), ["invalid_scale"], "{scale}");
        assert_eq!(app.state().export.png.scale, 1.0, "{scale}");
    }
    for scale in [0.5, 4.0] {
        png(&mut app, PngOp::SetScale { scale });
        assert!(refusals(&mut app).is_empty(), "{scale}");
    }
}

#[test]
fn a_frame_over_the_pixel_limit_says_the_size_it_would_have_and_the_largest_scale_that_fits() {
    let mut app = app_with(frame("Big", 3000, 3000), "f");
    png(&mut app, PngOp::SetScale { scale: 2.0 });
    assert_eq!(
        plan(&app),
        json!({"type": "refused", "code": "too_large",
               "asked": {"scale": 2.0, "width": 6000, "height": 6000},
               "fits": {"scale": 1.0, "width": 3000, "height": 3000}})
    );
    // 4096 by 4096 is the limit: exactly that fits, one pixel more does not.
    let mut edge = app_with(frame("Edge", 4096, 4096), "f");
    assert_eq!(plan(&edge)["type"], "ready");
    for (w, h) in [(4097, 4096), (4096, 4097)] {
        let over = app_with(frame("Over", w, h), "f");
        assert_eq!(plan(&over)["code"], "too_large", "{w}x{h}");
    }
    png(&mut edge, PngOp::SetScale { scale: 0.5 });
    assert_eq!(plan(&edge)["width"], 2048);
    let huge = app_with(frame("Huge", 20000, 20000), "f");
    let mut huge = huge;
    png(&mut huge, PngOp::SetScale { scale: 0.5 });
    assert_eq!(plan(&huge)["fits"], Value::Null, "not even 0.5x fits");
    assert_eq!(plan(&huge)["code"], "too_large");
}

#[test]
fn an_export_is_one_effect_with_the_rectangle_the_scale_the_size_the_file_and_where_it_goes() {
    let mut app = app_with(frame("Hero", 200, 100), "f");
    png(&mut app, PngOp::SetScale { scale: 3.0 });
    let fx = export(&mut app);
    assert_eq!(
        effects_json(&fx),
        vec![json!({"type": "raster", "request": 1,
            "rect": {"x": 100.0, "y": 50.0, "w": 200.0, "h": 100.0}, "scale": 3.0,
            "width": 600, "height": 300, "file": "Hero@3x.png", "to": {"type": "download"}})]
    );
}

#[test]
fn an_export_the_plan_refuses_draws_nothing() {
    let mut app = app_with(frame("Big", 3000, 3000), "f");
    png(&mut app, PngOp::SetScale { scale: 4.0 });
    app.take_events();
    assert!(export(&mut app).is_empty());
    assert_eq!(refusals(&mut app), ["too_large"]);
    // Only the frame the section or the dialog acts on can be exported.
    let list = json!([{"id": "f", "type": "frame", "w": 10, "h": 10}, {"id": "g", "type": "frame", "w": 10, "h": 10}]);
    let mut two = app_with(list, "f");
    let fx = png(
        &mut two,
        PngOp::Export {
            id: "g".into(),
            to: PngSink::Download,
        },
    );
    assert!(fx.is_empty());
    assert_eq!(refusals(&mut two), ["not_exportable"]);
}

#[test]
fn a_hostile_frame_name_never_reaches_the_effect_raw() {
    let hostile = format!(
        "../a/b\\c:d\u{202E}e\u{200B}f\u{3164}.png{}",
        "x".repeat(200)
    );
    let mut app = app_with(frame(&hostile, 10, 10), "f");
    png(&mut app, PngOp::SetScale { scale: 2.0 });
    let planned = plan(&app)["file"].as_str().unwrap().to_owned();
    let fx = effects_json(&export(&mut app));
    let file = fx[0]["file"].as_str().unwrap();
    assert_eq!(file, planned);
    assert!(file.ends_with("@2x.png"), "{file}");
    assert!(!file.contains(['/', '\\', ':']), "{file}");
    assert!(
        !file
            .chars()
            .any(|c| meldframe_text::is_hidden_text(c) || c.is_control()),
        "{file:?}"
    );
    assert!(file.chars().count() <= 80 + "@2x.png".len(), "{file}");
    assert!(file.starts_with("-a-b-c-d"), "{file}");
}

#[test]
fn the_plan_follows_the_document_and_is_worked_out_only_when_the_scene_moved() {
    let mut app = app_with(frame("Hero", 200, 100), "f");
    app.take_events();
    let (resolves, indexes) = (app.scene_resolves(), app.hit_builds());
    app.dispatch(Action::Resize {
        id: "f".into(),
        w: 300.0,
        h: 100.0,
        x: None,
        y: None,
    });
    assert_eq!(plan(&app)["width"], 300);
    assert!(app.take_events().contains(&Event::ExportChanged {}));
    assert_eq!(
        app.scene_resolves(),
        resolves + 1,
        "one scene for the new epoch"
    );
    assert_eq!(app.hit_builds(), indexes + 1, "and one index of it");
    // A dispatch that changes nothing the plan reads works nothing out and raises no event for it.
    let (resolves, indexes) = (app.scene_resolves(), app.hit_builds());
    app.dispatch(Action::SetPanel {
        panel: meldframe_app::Panel::Layers,
        open: true,
    });
    assert!(!app.take_events().contains(&Event::ExportChanged {}));
    assert_eq!(
        (app.scene_resolves(), app.hit_builds()),
        (resolves, indexes)
    );
}

/// A frame that hugs its one child: 999 by 999 as stored, 50 by 20 once laid out.
fn hug() -> Value {
    json!([
        {"id": "f", "type": "frame", "x": 0, "y": 0, "w": 999, "h": 999, "layout": "column", "padding": 0, "sw": "hug", "sh": "hug"},
        {"id": "a", "type": "rect", "parentId": "f", "pos": "a", "w": 50, "h": 20},
    ])
}

struct Fixed(f64, f64);

impl meldframe_app::TextMeasurer for Fixed {
    fn measure(&self, _: &Node, _: Option<f64>) -> (f64, f64) {
        (self.0, self.1)
    }
}

fn size_of_plan(app: &App) -> (Value, Value) {
    (plan(app)["width"].clone(), plan(app)["height"].clone())
}

#[test]
fn the_plan_moves_with_the_layout_flag_and_says_so() {
    let mut app = app_with(hug(), "f");
    assert_eq!(
        size_of_plan(&app),
        (json!(999), json!(999)),
        "the stored box"
    );
    app.set_layout_on_canvas(true);
    assert_eq!(
        size_of_plan(&app),
        (json!(50), json!(20)),
        "the laid-out box"
    );
    assert!(app.take_events().contains(&Event::ExportChanged {}));
    app.set_layout_on_canvas(false);
    assert_eq!(size_of_plan(&app), (json!(999), json!(999)));
    assert!(app.take_events().contains(&Event::ExportChanged {}));
    // An export reads the same plan.
    app.set_layout_on_canvas(true);
    let fx = export(&mut app);
    assert_eq!(effects_json(&fx)[0]["width"], 50);
}

#[test]
fn the_plan_moves_with_the_measurer_while_the_layout_is_on_and_not_while_it_is_off() {
    let list = json!([
        {"id": "f", "type": "frame", "x": 0, "y": 0, "w": 999, "h": 999, "layout": "column", "padding": 0, "sw": "hug", "sh": "hug"},
        {"id": "t", "type": "text", "parentId": "f", "pos": "a", "text": "hello", "sw": "hug", "sh": "hug"},
    ]);
    let mut app = app_with(list, "f");
    app.set_measurer(Box::new(Fixed(10.0, 10.0)));
    app.take_events();
    assert_eq!(
        size_of_plan(&app),
        (json!(999), json!(999)),
        "flag off: the stored box, whatever measures"
    );
    app.set_measurer(Box::new(Fixed(77.0, 33.0)));
    assert!(!app.take_events().contains(&Event::ExportChanged {}));
    app.set_layout_on_canvas(true);
    assert_eq!(size_of_plan(&app), (json!(77), json!(33)));
    app.take_events();
    app.set_measurer(Box::new(Fixed(80.0, 40.0)));
    assert_eq!(size_of_plan(&app), (json!(80), json!(40)));
    assert!(app.take_events().contains(&Event::ExportChanged {}));
}

#[test]
fn a_host_call_that_moves_the_dialog_and_the_plan_raises_one_export_event() {
    let list = json!([
        {"id": "f", "type": "frame", "x": 0, "y": 0, "w": 999, "h": 999, "layout": "column", "padding": 0, "sw": "hug", "sh": "hug"},
        {"id": "t", "type": "text", "parentId": "f", "pos": "a", "text": "hello", "sw": "hug", "sh": "hug"},
    ]);
    let mut app = app_with(list, "f");
    app.set_layout_on_canvas(true);
    app.set_measurer(Box::new(Fixed(10.0, 10.0)));
    app.dispatch(Action::OpenExport {});
    app.take_events();
    // The dialog is built again and the plan of the frame changes with the measurer: the front end reads the state once.
    app.set_measurer(Box::new(Fixed(80.0, 40.0)));
    assert_eq!(size_of_plan(&app), (json!(80), json!(40)));
    let changed = |events: Vec<Event>| {
        events
            .iter()
            .filter(|e| **e == Event::ExportChanged {})
            .count()
    };
    assert_eq!(changed(app.take_events()), 1);
    app.set_layout_on_canvas(false);
    assert_eq!(changed(app.take_events()), 1);
}

#[test]
fn the_sinks_of_a_raster_are_closed_shapes() {
    // `deny_unknown_fields` is not enforced on a unit variant of a tagged enum, so they are empty structs (as every action is).
    use meldframe_app::RasterSink;
    for kind in ["download", "clipboard"] {
        assert!(serde_json::from_value::<RasterSink>(json!({"type": kind})).is_ok());
        assert!(serde_json::from_value::<RasterSink>(json!({"type": kind, "x": 1})).is_err());
    }
}

#[test]
fn no_scale_makes_an_image_of_zero_pixels_or_one_that_wraps() {
    use meldframe_app_rules::raster::{Refusal, size_of};
    let rect = |w, h| RasterRect {
        x: 0.0,
        y: 0.0,
        w,
        h,
    };
    // A scale as small as a number gets (a capture may ask for any above 0) still draws one pixel, never none.
    assert_eq!(
        size_of(&rect(0.1, 0.1), f64::from_bits(1), 1 << 24),
        Ok((1, 1))
    );
    assert_eq!(size_of(&rect(1.0, 1e-300), 1e-300, 1 << 24), Ok((1, 1)));
    // A side that does not fit 32 bits is over the limit, whatever limit the caller gives: it never wraps or saturates.
    assert_eq!(
        size_of(&rect(1e10, 0.1), 1.0, u64::MAX),
        Err(Refusal::TooLarge)
    );
    assert_eq!(
        size_of(&rect(0.1, 5e9), 1.0, u64::MAX),
        Err(Refusal::TooLarge)
    );
    assert_eq!(
        size_of(&rect(4294967295.0, 1.0), 1.0, u64::MAX),
        Ok((u32::MAX, 1))
    );
}
