//! Drawing a node with a tool as an intent in world space (T8, decision 43): the box from two world points, the frame the node
//! lands in, what a click makes, one undo step, and that the preview is what the release makes.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::{CreateKind, Modifiers, Tool};
use meldframe_protocol::{Node, NodeKind};
use serde_json::json;

fn made(a: &meldframe_app::App, id: &str) -> (Option<String>, f64, f64, f64, f64) {
    let n = node(a, id);
    (n.parent_id, n.x, n.y, n.w, n.h)
}

#[test]
fn a_drag_on_the_canvas_makes_the_normalised_box_in_world_space() {
    let mut a = app(false);
    // To the left and up: the same box as the other way round.
    let id = draw(
        &mut a,
        CreateKind::Rectangle,
        (1300.0, 900.0),
        (1250.0, 860.0),
    )
    .unwrap();
    assert_eq!(made(&a, &id), (None, 1250.0, 860.0, 50.0, 40.0));
    assert_eq!(node(&a, &id).kind, NodeKind::Rect);
}

#[test]
fn each_tool_makes_its_kind() {
    let mut a = app(false);
    for (kind, want) in [
        (CreateKind::Frame, NodeKind::Frame),
        (CreateKind::Rectangle, NodeKind::Rect),
        (CreateKind::Ellipse, NodeKind::Ellipse),
        (CreateKind::Text, NodeKind::Text),
    ] {
        let id = draw(&mut a, kind, (2000.0, 0.0), (2040.0, 30.0)).unwrap();
        assert_eq!(node(&a, &id).kind, want);
    }
}

#[test]
fn a_node_drawn_in_a_nested_frame_is_placed_in_the_space_of_that_frame() {
    // `y` is a frame at world (135, 535) inside the free frames `m`, `x` and `tree`.
    let mut a = app_of(cards(), false);
    let id = draw(
        &mut a,
        CreateKind::Rectangle,
        (150.0, 560.0),
        (200.0, 600.0),
    )
    .unwrap();
    assert_eq!(made(&a, &id), (Some("y".into()), 15.0, 25.0, 50.0, 40.0));
}

#[test]
fn a_press_over_a_rectangle_picks_the_nearest_frame_above_it() {
    // `k` is a rectangle in the frame `c2` (laid out at world 220, 60), at (10, 10) in it: world (230, 70), 20 x 20.
    let mut a = app_of(cards(), true);
    let id = draw(&mut a, CreateKind::Ellipse, (235.0, 75.0), (255.0, 95.0)).unwrap();
    assert_eq!(made(&a, &id), (Some("c2".into()), 15.0, 15.0, 20.0, 20.0));
}

#[test]
fn a_press_over_no_frame_draws_on_the_canvas() {
    let mut a = app_of(cards(), false);
    let id = draw(&mut a, CreateKind::Frame, (50.0, 900.0), (90.0, 950.0)).unwrap();
    assert_eq!(made(&a, &id), (None, 50.0, 900.0, 40.0, 50.0));
}

#[test]
fn the_origin_of_the_frame_is_where_the_canvas_draws_it() {
    // `c1` is laid out by the flow of `deck` at world (110, 60); its stored offset is (0, 0).
    let mut on = app_of(cards(), true);
    let id = draw(&mut on, CreateKind::Rectangle, (120.0, 70.0), (140.0, 90.0)).unwrap();
    assert_eq!(made(&on, &id), (Some("c1".into()), 10.0, 10.0, 20.0, 20.0));
    // With the layout off the canvas draws the stored boxes: `c1`, `c2` and `c3` are all at the origin of `deck`, (100, 50),
    // one over the other, and the last one drawn is the one under the pointer.
    let mut off = app_of(cards(), false);
    let id = draw(
        &mut off,
        CreateKind::Rectangle,
        (120.0, 70.0),
        (140.0, 90.0),
    )
    .unwrap();
    assert_eq!(made(&off, &id), (Some("c3".into()), 20.0, 20.0, 20.0, 20.0));
}

#[test]
fn a_click_makes_a_node_of_the_default_size_at_the_point() {
    let mut a = app_of(cards(), false);
    let id = draw(
        &mut a,
        CreateKind::Rectangle,
        (140.0, 560.0),
        (140.0, 560.0),
    )
    .unwrap();
    let n = node(&a, &id);
    assert_eq!((n.x, n.y), (140.0 - 135.0, 560.0 - 535.0));
    let default: Node = serde_json::from_value(json!({"id": "d", "type": "rect"})).unwrap();
    assert_eq!((n.w, n.h), (default.w, default.h));
}

#[test]
fn a_box_dragged_flat_gets_the_smallest_size_on_that_side() {
    let mut a = app(false);
    let id = draw(
        &mut a,
        CreateKind::Rectangle,
        (1300.0, 900.0),
        (1360.0, 900.0),
    )
    .unwrap();
    assert_eq!(made(&a, &id), (None, 1300.0, 900.0, 60.0, 1.0));
    let id = draw(
        &mut a,
        CreateKind::Rectangle,
        (1300.0, 900.0),
        (1300.0, 960.0),
    )
    .unwrap();
    assert_eq!(made(&a, &id), (None, 1300.0, 900.0, 1.0, 60.0));
}

#[test]
fn a_draw_is_one_undo_step_that_selects_the_node_and_gives_the_select_tool_back() {
    let mut a = app(false);
    a.dispatch(meldframe_app::Action::SetTool {
        tool: Tool::Rectangle,
    });
    let before = a.nodes().to_vec();
    let id = draw(
        &mut a,
        CreateKind::Rectangle,
        (1300.0, 900.0),
        (1350.0, 940.0),
    )
    .unwrap();
    assert_eq!(steps(&a), 1);
    assert_eq!(a.state().selection, vec![id]);
    assert_eq!(a.state().tool, Tool::Select);
    a.dispatch(meldframe_app::Action::Undo {});
    assert_eq!(snapshot(a.nodes()), snapshot(&before));
}

#[test]
fn a_point_that_is_not_a_number_is_refused_and_nothing_is_made() {
    let mut a = app(false);
    let before = a.nodes().to_vec();
    for p in [f64::NAN, f64::INFINITY] {
        assert_eq!(
            draw(&mut a, CreateKind::Rectangle, (p, 0.0), (10.0, 10.0)),
            None
        );
        assert_eq!(
            draw(&mut a, CreateKind::Rectangle, (0.0, 0.0), (10.0, p)),
            None
        );
    }
    assert_eq!(refusals(&mut a), vec!["invalid"; 4]);
    assert_eq!(snapshot(a.nodes()), snapshot(&before));
    assert_eq!(steps(&a), 0);
}

#[test]
fn keys_held_change_no_geometry_yet() {
    // The web editor never read a modifier for a draw; the rules are its own, moved (`Modifiers`). With and without the keys,
    // the preview and the node that is made are the same.
    let (from, to) = ((1300.0, 900.0), (1350.0, 940.0));
    let held = Modifiers {
        shift: true,
        alt: true,
        ctrl: true,
        meta: true,
    };
    let mut plain = app(false);
    let mut keys = app(false);
    assert_eq!(
        plain.draw_preview(
            CreateKind::Rectangle,
            pt(from.0, from.1),
            pt(to.0, to.1),
            Modifiers::default()
        ),
        keys.draw_preview(
            CreateKind::Rectangle,
            pt(from.0, from.1),
            pt(to.0, to.1),
            held
        )
    );
    let a = draw(&mut plain, CreateKind::Rectangle, from, to).unwrap();
    let b = draw_with(&mut keys, CreateKind::Rectangle, from, to, held).unwrap();
    assert_eq!(made(&plain, &a), made(&keys, &b));
}

#[test]
fn the_frame_is_the_one_at_the_press_not_the_one_where_the_pointer_is_let_go() {
    // `other` is a free frame at (100, 400), 300 x 100; the canvas around it holds no frame.
    let mut a = app_of(cards(), false);
    // From the empty canvas into the frame: the canvas, in world space.
    let id = draw(&mut a, CreateKind::Rectangle, (50.0, 420.0), (150.0, 450.0)).unwrap();
    assert_eq!(made(&a, &id), (None, 50.0, 420.0, 100.0, 30.0));
    // From the frame out to the canvas: the frame, in its space (a box that leaves it is not cut).
    let id = draw(&mut a, CreateKind::Rectangle, (150.0, 420.0), (50.0, 450.0)).unwrap();
    assert_eq!(
        made(&a, &id),
        (Some("other".into()), -50.0, 20.0, 100.0, 30.0)
    );
}

#[test]
fn the_preview_says_where_the_box_is_on_the_canvas() {
    let a = app_of(cards(), false);
    // In the frame `y` (world 135, 535): the world box is the dragged one, the local box is in the space of `y`.
    let p = a
        .draw_preview(
            CreateKind::Frame,
            pt(200.0, 600.0),
            pt(150.0, 560.0),
            Modifiers::default(),
        )
        .unwrap();
    assert_eq!(p.parent.as_deref(), Some("y"));
    let (w, l) = (p.world, p.local);
    assert_eq!((w.x, w.y, w.w, w.h), (150.0, 560.0, 50.0, 40.0));
    assert_eq!((l.x, l.y, l.w, l.h), (15.0, 25.0, 50.0, 40.0));
    assert!(!p.click);
    // A click: the default size of the kind at the point.
    let p = a
        .draw_preview(
            CreateKind::Rectangle,
            pt(140.0, 560.0),
            pt(140.0, 560.0),
            Modifiers::default(),
        )
        .unwrap();
    let default: Node = serde_json::from_value(json!({"id": "d", "type": "rect"})).unwrap();
    assert!(p.click);
    assert_eq!(
        (p.world.x, p.world.y, p.world.w, p.world.h),
        (140.0, 560.0, default.w, default.h)
    );
    assert_eq!((p.local.x, p.local.y), (5.0, 25.0));
}

#[test]
fn a_draw_raises_one_document_change_and_a_refused_one_none() {
    let mut a = app(false);
    a.take_events();
    draw(&mut a, CreateKind::Frame, (2000.0, 0.0), (2040.0, 30.0)).unwrap();
    assert_eq!(document_changes(&mut a), 1);
    draw(&mut a, CreateKind::Frame, (f64::NAN, 0.0), (2040.0, 30.0));
    assert_eq!(document_changes(&mut a), 0);
}

#[test]
fn a_preview_is_none_where_the_release_is_refused() {
    // The file holds coordinates up to 1,000,000 (`meldframe_check::Limits`): a box that goes past them is refused.
    for on in [false, true] {
        let mut a = app(on);
        for (from, to) in [
            ((0.0, 0.0), (2e6, 10.0)),
            ((2e6, 0.0), (2e6 + 50.0, 10.0)),
            ((0.0, -2e6), (10.0, 10.0)),
            ((0.0, 0.0), (f64::NAN, 10.0)),
        ] {
            let p = a.draw_preview(
                CreateKind::Rectangle,
                pt(from.0, from.1),
                pt(to.0, to.1),
                Modifiers::default(),
            );
            let made = draw(&mut a, CreateKind::Rectangle, from, to);
            assert!(
                p.is_none() && made.is_none(),
                "{from:?} to {to:?}, layout {on}"
            );
        }
        assert_eq!(refusals(&mut a), vec!["invalid"; 4]);
    }
}

#[test]
fn a_node_made_in_a_flow_is_placed_by_the_layout_only_with_the_layout_on() {
    // `f` is a column at the origin; press in it where nothing else is.
    for on in [false, true] {
        let mut a = app(on);
        let (from, to) = ((100.0, 200.0), (130.0, 230.0));
        let p = a
            .draw_preview(
                CreateKind::Rectangle,
                pt(from.0, from.1),
                pt(to.0, to.1),
                Modifiers::default(),
            )
            .unwrap();
        assert_eq!(p.parent.as_deref(), Some("f"));
        assert_eq!(p.placed_by_layout, on);
        let id = draw(&mut a, CreateKind::Rectangle, from, to).unwrap();
        let drawn = box_of(&a, &id);
        let at = (p.local.x, p.local.y, p.local.w, p.local.h);
        if p.placed_by_layout {
            assert_ne!(
                (drawn.0, drawn.1),
                (at.0, at.1),
                "the flow puts it elsewhere"
            );
        } else {
            assert_eq!(drawn, at);
        }
    }
    // A free frame and the canvas are never placed by a layout.
    let a = app(true);
    for from in [(450.0, 150.0), (1500.0, 900.0)] {
        let p = a
            .draw_preview(
                CreateKind::Rectangle,
                pt(from.0, from.1),
                pt(from.0 + 10.0, from.1 + 10.0),
                Modifiers::default(),
            )
            .unwrap();
        assert!(!p.placed_by_layout);
    }
}

#[test]
fn the_preview_is_what_the_release_makes() {
    let cases = [
        ((150.0, 560.0), (200.0, 600.0)),
        ((235.0, 75.0), (200.0, 60.0)),
        ((140.0, 560.0), (140.0, 560.0)),
        ((900.0, 900.0), (960.0, 900.0)),
    ];
    for on in [false, true] {
        for (from, to) in cases {
            let mut a = app_of(cards(), on);
            let steps_before = steps(&a);
            let p = a
                .draw_preview(
                    CreateKind::Rectangle,
                    pt(from.0, from.1),
                    pt(to.0, to.1),
                    Modifiers::default(),
                )
                .unwrap();
            assert_eq!(steps(&a), steps_before, "a preview writes nothing");
            let id = draw(&mut a, CreateKind::Rectangle, from, to).unwrap();
            let n = node(&a, &id);
            assert_eq!(n.parent_id, p.parent);
            assert_eq!(
                (n.x, n.y, n.w, n.h),
                (p.local.x, p.local.y, p.local.w, p.local.h)
            );
            // The flow of an auto layout frame places what is made in it, so the drawn box is the preview's only elsewhere.
            if !p.placed_by_layout {
                let b = box_of(&a, &id);
                assert_eq!(b, (p.local.x, p.local.y, p.local.w, p.local.h));
            }
            assert_eq!(p.click, from == to);
            // On the canvas the box is where the pointer drew it, whatever the frame.
            assert_eq!((p.world.x, p.world.y), (from.0.min(to.0), from.1.min(to.1)));
            assert_eq!((p.world.w, p.world.h), (p.local.w, p.local.h));
        }
    }
}

#[test]
fn the_preview_reads_no_state_of_the_editor() {
    let mut a = app(false);
    a.dispatch(meldframe_app::Action::Select {
        ids: vec!["a".into()],
    });
    a.take_events();
    let state = serde_json::to_value(a.state()).unwrap();
    a.draw_preview(
        CreateKind::Frame,
        pt(0.0, 0.0),
        pt(30.0, 30.0),
        Modifiers::default(),
    );
    a.resize_preview(
        "a",
        meldframe_app::Handle::E,
        pt(5.0, 0.0),
        Modifiers::default(),
    );
    assert_eq!(serde_json::to_value(a.state()).unwrap(), state);
    assert!(a.take_events().is_empty());
}
