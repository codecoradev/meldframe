//! The app keeps the scene the canvas draws for as long as nothing it is made from has changed, and says when it did: a native
//! shell that calls the app as a library redraws when `App::scene_epoch` moves, and a spike found that resolving the layout on
//! every frame is the one thing such a client must not do. `App::scene_resolves` counts how often the scene was worked out.
//! The app holds no camera (the scene is in world space), so a camera is not an input of either epoch.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::{Action, App, Panel, Tool};
use meldframe_protocol::{Doc, Node};
use serde_json::json;

fn mode_app() -> App {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "root", "type": "frame", "pos": "a0", "w": 200, "h": 100, "fill": {"var": "bg"}},
    ]))
    .unwrap();
    let vars = serde_json::from_value(json!({
        "collections": [{"id": "t", "name": "appearance", "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color", "values": {"light": "#ffffff", "dark": "#101112"}}],
    }))
    .unwrap();
    App::with_doc("user:me", "c0", 1, Doc { nodes, vars }, 0).unwrap()
}

fn resize_c(a: &mut App, h: f64) {
    a.dispatch(Action::Resize {
        id: "c".into(),
        w: 50.0,
        h,
        x: None,
        y: None,
    });
}

#[test]
fn two_reads_with_no_change_between_them_share_an_epoch_and_resolve_once() {
    for on in [true, false] {
        let a = app(on);
        let epoch = a.scene_epoch();
        assert_eq!(a.scene_resolves(), 0, "asking the epoch resolves nothing");
        let first = a.scene();
        assert_eq!(a.scene_resolves(), 1);
        let second = a.scene();
        assert_eq!(first, second);
        assert_eq!(a.scene_epoch(), epoch, "layout {on}");
        assert_eq!(a.scene_resolves(), 1, "layout {on}: no work for the second");
        a.with_scene(|e, boxes| {
            assert_eq!(e, epoch);
            assert_eq!(boxes, first.as_slice());
            // The read may ask again.
            assert_eq!(a.scene_epoch(), e);
        });
        assert_eq!(a.scene_resolves(), 1);
        // A hit builds its index from the kept scene.
        a.hit(30.0, 12.0);
        assert_eq!(a.scene_resolves(), 1, "the index reads the scene held");
    }
}

#[test]
fn an_edit_moves_both_epochs_and_the_scene_is_resolved_again_once() {
    let mut a = app(true);
    let (scene, hits) = (a.scene_epoch(), a.hit_index_epoch());
    a.scene();
    resize_c(&mut a, 90.0);
    assert!(a.scene_epoch() > scene);
    assert!(a.hit_index_epoch() > hits);
    let moved = a.scene_epoch();
    a.scene();
    a.scene();
    assert_eq!(a.scene_epoch(), moved);
    assert_eq!(a.scene_resolves(), 2, "one for each document");
    assert_eq!(box_of(&a, "c").3, 90.0, "the new scene, not the kept one");
}

#[test]
fn the_flag_and_the_measurer_move_both_epochs() {
    let mut a = app(false);
    let (scene, hits) = (a.scene_epoch(), a.hit_index_epoch());
    a.set_layout_on_canvas(true);
    assert!(a.scene_epoch() > scene && a.hit_index_epoch() > hits);
    let (scene, hits) = (a.scene_epoch(), a.hit_index_epoch());
    a.set_measurer(Box::new(meldframe_app::StubMeasurer));
    assert!(a.scene_epoch() > scene && a.hit_index_epoch() > hits);
}

#[test]
fn a_mode_moves_the_scene_epoch_and_leaves_the_hit_index_epoch_alone() {
    let mut a = mode_app();
    let (scene, hits) = (a.scene_epoch(), a.hit_index_epoch());
    let light = a.scene();
    a.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    assert!(a.scene_epoch() > scene, "the painted colour changed");
    assert_ne!(a.scene(), light);
    assert_eq!(a.hit_index_epoch(), hits, "no box moved");
}

#[test]
fn what_is_not_the_document_the_measurer_the_flag_or_a_mode_moves_nothing() {
    let mut a = app(true);
    a.scene();
    a.hit(30.0, 12.0);
    let (scene, hits, resolves) = (a.scene_epoch(), a.hit_index_epoch(), a.scene_resolves());
    // The app holds no camera: pan and zoom are the shell's, so a shell that moves its camera calls nothing here. What the
    // app does hold of the view (selection, tool, panels, time) is not an input either.
    a.dispatch(Action::Select {
        ids: vec!["a".into()],
    });
    a.dispatch(Action::SetTool { tool: Tool::Frame });
    a.dispatch(Action::TogglePanel {
        panel: Panel::Layers,
    });
    a.dispatch(Action::Tick { now_ms: 5_000 });
    a.scene();
    a.hit(30.0, 12.0);
    assert_eq!(a.scene_epoch(), scene);
    assert_eq!(a.hit_index_epoch(), hits);
    assert_eq!(a.scene_resolves(), resolves);
    assert_eq!(a.hit_builds(), 1);
}

/// A file of `n` nodes for the timing: one root, then frames of 99 rectangles each.
#[cfg(not(target_arch = "wasm32"))]
fn big(n: usize) -> App {
    let mut list = vec![json!({"id": "root", "type": "frame", "pos": "a0", "w": 4000, "h": 4000})];
    let (mut i, mut f) = (1, 0);
    while i < n {
        let id = format!("f{f}");
        list.push(json!({"id": id, "type": "frame", "parentId": "root", "pos": format!("a{f:05}"), "w": 300, "h": 2000, "layout": "column"}));
        i += 1;
        for k in 0..99.min(n - i) {
            list.push(json!({"id": format!("{id}k{k}"), "type": "rect", "parentId": id, "pos": format!("a{k:03}"), "w": 50, "h": 10}));
        }
        i += 99.min(n - i);
        f += 1;
    }
    app_of(serde_json::from_value(list.into()).unwrap(), true)
}

/// Run by hand: `cargo test --release -p meldframe-app-layout-tests --test scene_epoch -- --ignored --nocapture`. It prints
/// the milliseconds to resolve the scene (layout on, stub measurer) after an edit, and to read the held scene again.
#[test]
#[ignore = "prints timings; run by hand in a release build"]
#[cfg(not(target_arch = "wasm32"))]
fn time_the_scene_at_one_five_and_twenty_thousand_nodes() {
    for n in [1_000, 5_000, 19_999] {
        let mut a = big(n);
        assert_eq!(a.nodes().len(), n);
        let t = std::time::Instant::now();
        a.scene();
        let resolve = t.elapsed();
        let t = std::time::Instant::now();
        a.with_scene(|_, boxes| boxes.len());
        let held = t.elapsed();
        let t = std::time::Instant::now();
        a.scene_epoch();
        let epoch = t.elapsed();
        a.dispatch(Action::Resize {
            id: "f0k0".into(),
            w: 60.0,
            h: 10.0,
            x: None,
            y: None,
        });
        let t = std::time::Instant::now();
        a.scene();
        let after_edit = t.elapsed();
        eprintln!(
            "{n} nodes: first resolve {resolve:?}, resolve after an edit {after_edit:?}, read of the held scene {held:?}, epoch {epoch:?}"
        );
    }
}
