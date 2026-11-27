//! The run palette driven through the app with no front end (issue E8.1.5): a query gives the entries, an entry is run by its
//! id, and what it does is what the key or the button for the same thing does. The palette closes before the entry acts, as the
//! web palette does. PaletteZoom is the one run the app cannot do yet (the camera is the front end's until E8.1.4): it is handed back.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::keys::*;
use common::*;
use meldframe_app::{Action, App, Modifiers, Overlay, OverlayOp, PaletteRan, PaletteZoom, Tool};
use meldframe_app_rules::keys::{Held, TABLE};

fn found(app: &App, query: &str) -> Vec<String> {
    app.palette(query, false)
        .into_iter()
        .map(|e| e.id)
        .collect()
}

fn enabled(app: &App, id: &str) -> bool {
    app.palette("", false)
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .enabled
}

fn open_palette(app: &mut App) {
    app.dispatch(Action::Overlay {
        op: OverlayOp::Open {
            overlay: Overlay::Palette,
        },
    });
    assert_eq!(app.state().overlay, Some(Overlay::Palette));
}

fn done(r: PaletteRan) -> Vec<meldframe_app::Effect> {
    match r {
        PaletteRan::Done(fx) => fx,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_query_narrows_the_entries_and_the_state_decides_which_can_run() {
    let mut a = app();
    assert_eq!(found(&a, "undo"), ["undo"]);
    assert!(!enabled(&a, "undo") && !enabled(&a, "delete") && !enabled(&a, "export"));
    a.dispatch(Action::Move {
        id: "r1".into(),
        x: 40.0,
        y: 0.0,
    });
    a.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    assert!(
        enabled(&a, "undo")
            && enabled(&a, "delete")
            && enabled(&a, "export")
            && enabled(&a, "variant")
    );
    // A word that no entry has leaves none, and an entry that cannot run is listed after those that can.
    assert!(found(&a, "zoom banana").is_empty());
    assert_eq!(found(&a, "do"), ["undo", "redo"]);
    a.dispatch(Action::Undo {});
    assert_eq!(found(&a, "do"), ["redo", "undo"]);
}

#[test]
fn running_an_entry_closes_the_palette_and_does_what_the_entry_says() {
    let mut a = app();
    a.dispatch(Action::Connected { now_ms: 0 });
    open_palette(&mut a);
    done(a.run_palette("tool-rectangle", (0.0, 0.0)));
    assert_eq!(a.state().tool, Tool::Rectangle);
    assert_eq!(a.state().overlay, None);

    a.dispatch(Action::Select { ids: ids(&["r1"]) });
    open_palette(&mut a);
    let fx = done(a.run_palette("delete", (0.0, 0.0)));
    assert!(a.nodes().iter().all(|n| n.id != "r1"));
    assert_eq!(sent(&fx).len(), 1);
    done(a.run_palette("undo", (0.0, 0.0)));
    assert!(a.nodes().iter().any(|n| n.id == "r1"));
}

#[test]
fn an_entry_that_opens_a_dialog_leaves_the_dialog_open_and_the_palette_shut() {
    let mut a = app();
    a.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    open_palette(&mut a);
    done(a.run_palette("export", (0.0, 0.0)));
    assert!(a.state().export.dialog.is_some());
    assert_eq!(a.state().overlay, None);

    let mut b = app();
    open_palette(&mut b);
    done(b.run_palette("shortcuts", (0.0, 0.0)));
    assert_eq!(b.state().overlay, Some(Overlay::Help));
}

#[test]
fn the_switch_entry_turns_auto_layout_on_canvas_on_and_off_and_says_which() {
    let mut a = app();
    let detail = |a: &App| a.palette("auto layout", false)[0].detail.clone().unwrap();
    assert!(!a.layout_on_canvas() && detail(&a).starts_with("Off"));
    done(a.run_palette("auto-layout", (0.0, 0.0)));
    assert!(a.layout_on_canvas() && detail(&a).starts_with("On"));
    done(a.run_palette("auto-layout", (0.0, 0.0)));
    assert!(!a.layout_on_canvas());
}

#[test]
fn a_preset_makes_a_frame_centred_on_the_point_the_front_end_gives() {
    let mut a = app();
    a.dispatch(Action::Connected { now_ms: 0 });
    done(a.run_palette("preset-a4", (100.0, 100.0)));
    let id = a.state().selection[0].clone();
    let n = get(&a, &id);
    // 100 - 297.5 and 100 - 421, rounded the way the web front end rounds (half up), as one frame the person can undo.
    assert_eq!((n.x, n.y, n.w, n.h), (-197.0, -321.0, 595.0, 842.0));
    assert_eq!(a.state().tool, Tool::Select);
}

#[test]
fn zoom_is_handed_back_to_the_front_end_and_a_refused_entry_changes_nothing() {
    let mut a = app();
    assert!(matches!(
        a.run_palette("zoom-fit", (0.0, 0.0)),
        PaletteRan::Host(PaletteZoom::Fit)
    ));
    a.take_events();
    let before = a.state().clone();
    for id in ["undo", "delete", "export", "variant", "nowhere"] {
        assert!(
            matches!(a.run_palette(id, (0.0, 0.0)), PaletteRan::Refused),
            "{id}"
        );
    }
    assert_eq!(*a.state(), before);
    assert!(a.take_events().is_empty());
}

/// An entry and the key of its row do the same to the same state: the palette and the handlers read one table.
#[test]
fn an_entry_does_what_the_key_shown_beside_it_does() {
    fn row_of(entry: &str) -> &str {
        match entry {
            "toggle-layers" => "layers-panel",
            "toggle-inspector" => "right-panel",
            other => other,
        }
    }
    let prepared = |id: &str| {
        let mut a = app();
        a.dispatch(Action::Connected { now_ms: 0 });
        a.dispatch(Action::Move {
            id: "r1".into(),
            x: 40.0,
            y: 0.0,
        });
        if id == "redo" {
            a.dispatch(Action::Undo {});
        }
        a.dispatch(Action::Select { ids: ids(&["r1"]) });
        if id == "tool-select" {
            // From the select tool the key and the entry would both change nothing: start from another one (after the
            // selection, which goes back to the select tool).
            a.dispatch(Action::SetTool {
                tool: Tool::Rectangle,
            });
        }
        a
    };
    let shown: Vec<String> = app()
        .palette("", false)
        .into_iter()
        .filter(|e| e.shortcut.is_some())
        .map(|e| e.id)
        .collect();
    assert!(shown.len() >= 10, "{shown:?}");
    for id in shown.iter().filter(|i| !i.starts_with("zoom")) {
        let combo = TABLE
            .iter()
            .flat_map(|g| g.rows)
            .find(|r| r.id == row_of(id))
            .unwrap()
            .bindings[0]
            .combo;
        let m = Modifiers {
            ctrl: combo.command,
            meta: false,
            alt: combo.alt == Held::Yes,
            shift: combo.shift == Held::Yes,
        };
        let (mut by_entry, mut by_key) = (prepared(id), prepared(id));
        let before = by_entry.state().clone();
        done(by_entry.run_palette(id, (0.0, 0.0)));
        by_key.dispatch(press(combo.key, combo.code.unwrap_or(""), m));
        assert_ne!(*by_entry.state(), before, "{id}: the entry changed nothing");
        assert_eq!(by_entry.state(), by_key.state(), "{id}");
        assert_eq!(
            serde_json::to_value(by_entry.nodes()).unwrap(),
            serde_json::to_value(by_key.nodes()).unwrap(),
            "{id}"
        );
    }
}

#[test]
fn a_point_that_is_not_a_number_refuses_the_entry_before_the_palette_closes() {
    for centre in [
        (f64::NAN, 0.0),
        (0.0, f64::INFINITY),
        (f64::NEG_INFINITY, 1.0),
    ] {
        let mut a = app();
        open_palette(&mut a);
        a.take_events();
        let before = a.state().clone();
        assert!(matches!(
            a.run_palette("preset-a4", centre),
            PaletteRan::Refused
        ));
        assert_eq!(*a.state(), before);
        assert!(a.take_events().is_empty());
    }
}

#[test]
fn each_entry_that_runs_does_what_it_says() {
    let mut a = app();
    a.dispatch(Action::Connected { now_ms: 0 });
    a.dispatch(Action::Move {
        id: "r1".into(),
        x: 40.0,
        y: 0.0,
    });
    a.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    // Redo after an undo; the right panel closes and opens; a variant is a copy of the selected frame, selected.
    done(a.run_palette("undo", (0.0, 0.0)));
    assert_eq!(get(&a, "r1").x, 0.0);
    done(a.run_palette("redo", (0.0, 0.0)));
    assert_eq!(get(&a, "r1").x, 40.0);
    assert!(a.state().panels.properties);
    done(a.run_palette("toggle-inspector", (0.0, 0.0)));
    assert!(!a.state().panels.properties);
    done(a.run_palette("toggle-inspector", (0.0, 0.0)));
    assert!(a.state().panels.properties);
    let count = a.nodes().len();
    done(a.run_palette("variant", (0.0, 0.0)));
    assert!(a.nodes().len() > count);
    assert_ne!(a.state().selection, ids(&["root"]));
    for (id, zoom) in [
        ("zoom-in", PaletteZoom::In),
        ("zoom-out", PaletteZoom::Out),
        ("zoom-reset", PaletteZoom::Reset),
        ("zoom-fit", PaletteZoom::Fit),
        ("zoom-fit-selection", PaletteZoom::FitSelection),
    ] {
        match a.run_palette(id, (0.0, 0.0)) {
            PaletteRan::Host(z) => assert_eq!(z, zoom, "{id}"),
            other => panic!("{id}: {other:?}"),
        }
    }
}
