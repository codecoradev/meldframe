//! A resize by handle as an intent in world space (T8, decision 43): each of the eight handles, the smallest size, hug and
//! fill sides that become fixed, a flow child whose origin is the layout's, one undo step, and the preview.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::{Action, Handle, Modifiers};
use meldframe_protocol::SizeMode;

/// Where the space of the parent of `id` starts on the canvas, from the scene (the sum of the offsets up the chain).
fn origin_of(a: &meldframe_app::App, id: &str) -> (f64, f64) {
    let scene = a.scene();
    let mut at = (0.0, 0.0);
    let mut cur = scene
        .iter()
        .find(|b| b.id == id)
        .and_then(|b| b.parent.clone());
    while let Some(p) = cur {
        let b = scene.iter().find(|b| b.id == p).unwrap();
        at = (at.0 + b.x, at.1 + b.y);
        cur = b.parent.clone();
    }
    at
}

fn local(a: &meldframe_app::App, id: &str) -> (f64, f64, f64, f64) {
    let n = node(a, id);
    (n.x, n.y, n.w, n.h)
}

#[test]
fn each_handle_moves_its_edges_and_keeps_the_opposite_ones() {
    // `fg` is a free rectangle at (20, 20), 30 x 30, in `g`.
    let by = (5.0, 7.0);
    let table = [
        (Handle::Nw, (25.0, 27.0, 25.0, 23.0)),
        (Handle::N, (20.0, 27.0, 30.0, 23.0)),
        (Handle::Ne, (20.0, 27.0, 35.0, 23.0)),
        (Handle::E, (20.0, 20.0, 35.0, 30.0)),
        (Handle::Se, (20.0, 20.0, 35.0, 37.0)),
        (Handle::S, (20.0, 20.0, 30.0, 37.0)),
        (Handle::Sw, (25.0, 20.0, 25.0, 37.0)),
        (Handle::W, (25.0, 20.0, 25.0, 30.0)),
    ];
    for on in [false, true] {
        for (handle, want) in table {
            let mut a = app(on);
            drag_handle(&mut a, "fg", handle, by);
            assert_eq!(local(&a, "fg"), want, "{handle:?}, layout {on}");
            assert_eq!(steps(&a), 1);
        }
    }
}

/// The edges a handle holds, `(west, east, north, south)`, from its name (not from `Handle::edges`, which the rule uses).
fn held(handle: Handle) -> (bool, bool, bool, bool) {
    let name = format!("{handle:?}").to_lowercase();
    (
        name.contains('w'),
        name.contains('e'),
        name.contains('n'),
        name.contains('s'),
    )
}

const ALL: [Handle; 8] = [
    Handle::Nw,
    Handle::N,
    Handle::Ne,
    Handle::E,
    Handle::Se,
    Handle::S,
    Handle::Sw,
    Handle::W,
];

#[test]
fn a_drag_below_the_smallest_size_keeps_the_opposite_edge_for_every_handle() {
    // `fg` is a free rectangle at (20, 20), 30 x 30: its edges are at 20 and 50 on both axes.
    for on in [false, true] {
        for handle in ALL {
            let (west, east, north, south) = held(handle);
            // Each held edge is dragged past the opposite one, a side that is not held is dragged by nothing at all.
            let by = (
                if west {
                    100.0
                } else if east {
                    -100.0
                } else {
                    0.0
                },
                if north {
                    100.0
                } else if south {
                    -100.0
                } else {
                    0.0
                },
            );
            let mut a = app(on);
            drag_handle(&mut a, "fg", handle, by);
            // A held side is 1 long and ends at the edge that stayed; a side that is not held is as it was.
            let x = if west {
                (49.0, 1.0)
            } else if east {
                (20.0, 1.0)
            } else {
                (20.0, 30.0)
            };
            let y = if north {
                (49.0, 1.0)
            } else if south {
                (20.0, 1.0)
            } else {
                (20.0, 30.0)
            };
            assert_eq!(
                local(&a, "fg"),
                (x.0, y.0, x.1, y.1),
                "{handle:?}, layout {on}"
            );
            assert_eq!(steps(&a), 1);
        }
    }
}

#[test]
fn a_side_the_handle_does_not_hold_is_never_clamped() {
    // An empty hug frame is drawn 0 x 0 with the layout on: a drag that holds one side changes that side only.
    let mut list = nodes();
    list.extend(
        serde_json::from_value::<Vec<meldframe_protocol::Node>>(serde_json::json!([
            {"id": "emp", "type": "frame", "pos": "a8", "x": 2000, "y": 0, "w": 1, "h": 1,
             "sw": "hug", "sh": "hug", "layout": "column", "padding": 0},
            {"id": "thin", "type": "rect", "pos": "a9", "x": 2100, "y": 0, "w": 0.5, "h": 40},
        ]))
        .unwrap(),
    );
    let mut a = app_of(list, true);
    assert_eq!(box_of(&a, "emp").2, 0.0, "drawn 0 wide");
    for (id, handle, by) in [
        ("emp", Handle::S, (0.0, 0.0)),
        ("emp", Handle::E, (0.0, 0.0)),
        ("emp", Handle::Se, (0.0, 0.0)),
        ("thin", Handle::W, (0.0, 0.0)),
        ("thin", Handle::Nw, (0.0, 0.0)),
        ("thin", Handle::S, (0.0, 0.0)),
    ] {
        drag_handle(&mut a, id, handle, by);
        assert_eq!(steps(&a), 0, "{id} {handle:?}: a drag that changes nothing");
        assert!(a.take_events().is_empty(), "{id} {handle:?}");
    }
    assert_eq!(
        (node(&a, "emp").sw, node(&a, "emp").sh),
        (SizeMode::Hug, SizeMode::Hug)
    );
    assert_eq!(local(&a, "thin"), (2100.0, 0.0, 0.5, 40.0));
    // A drag that moves the held side leaves the other side as drawn: the south handle of the empty frame by 10.
    drag_handle(&mut a, "emp", Handle::S, (0.0, 10.0));
    let e = node(&a, "emp");
    assert_eq!((e.sh, e.h, e.sw), (SizeMode::Fixed, 10.0, SizeMode::Hug));
    assert_eq!(box_of(&a, "emp").2, 0.0, "the width is still the hug width");
    // The west handle of the thin one by 0.25 would make it 0.75 wide: that is a side the handle holds and moves, so it is 1
    // wide and ends at the right edge that stayed (2100.5); the height is as it was.
    drag_handle(&mut a, "thin", Handle::W, (-0.25, 0.0));
    assert_eq!(local(&a, "thin"), (2099.5, 0.0, 1.0, 40.0));
}

#[test]
fn a_hug_or_fill_side_becomes_fixed_only_when_its_size_changes() {
    let mut a = app_of(with_sized_b("fill"), true);
    // The east handle changes the width (drawn 180): fixed. The height is hug and not touched: stays.
    drag_handle(&mut a, "b", Handle::E, (10.0, 0.0));
    let b = node(&a, "b");
    assert_eq!((b.sw, b.w, b.sh), (SizeMode::Fixed, 190.0, SizeMode::Hug));
    assert_eq!(steps(&a), 1, "size and mode in one step");
    let mut a = app_of(with_sized_b("fill"), true);
    drag_handle(&mut a, "b", Handle::Se, (10.0, 5.0));
    let b = node(&a, "b");
    assert_eq!((b.sw, b.sh, b.h), (SizeMode::Fixed, SizeMode::Fixed, 35.0));
}

#[test]
fn the_origin_of_a_flow_child_is_left_alone() {
    let mut a = app_of(with_sized_b("fill"), true);
    // The west handle moves the left edge by -4 (the box is drawn from x 10): a flow child writes no position.
    drag_handle(&mut a, "b", Handle::Nw, (-4.0, -4.0));
    let b = node(&a, "b");
    assert_eq!((b.x, b.y), (0.0, 0.0));
    assert_eq!((b.w, b.h), (184.0, 34.0));
}

#[test]
fn with_the_layout_off_a_flow_child_moves_like_any_node() {
    let mut a = app_of(with_sized_b("fill"), false);
    drag_handle(&mut a, "b", Handle::Nw, (-4.0, -4.0));
    let b = node(&a, "b");
    assert_eq!((b.x, b.y, b.sw), (-4.0, -4.0, SizeMode::Fill));
}

#[test]
fn a_drag_that_changes_nothing_makes_no_step() {
    let mut a = app(true);
    drag_handle(&mut a, "fg", Handle::E, (0.0, 0.0));
    drag_handle(&mut a, "fg", Handle::N, (9.0, 0.0));
    assert_eq!(steps(&a), 0);
    assert!(refusals(&mut a).is_empty());
}

#[test]
fn an_unknown_node_and_a_number_that_is_not_one_are_refused() {
    let mut a = app(true);
    drag_handle(&mut a, "ghost", Handle::E, (5.0, 0.0));
    assert_eq!(refusals(&mut a), vec!["not_found"]);
    for by in [(f64::NAN, 0.0), (0.0, f64::INFINITY), (f64::MAX, 0.0)] {
        drag_handle(&mut a, "fg", Handle::Se, by);
    }
    assert_eq!(refusals(&mut a), vec!["invalid"; 3]);
    assert_eq!((steps(&a), local(&a, "fg")), (0, (20.0, 20.0, 30.0, 30.0)));
}

#[test]
fn one_drag_is_one_undo_step() {
    let mut a = app_of(with_sized_b("fill"), true);
    let before = a.nodes().to_vec();
    drag_handle(&mut a, "b", Handle::Se, (10.0, 5.0));
    assert_eq!(steps(&a), 1);
    a.dispatch(Action::Undo {});
    assert_eq!(snapshot(a.nodes()), snapshot(&before));
}

#[test]
fn the_preview_is_the_box_the_release_leaves() {
    for on in [false, true] {
        for (id, handle, by) in [
            ("fg", Handle::Nw, (5.0, 7.0)),
            ("fg", Handle::E, (-100.0, 0.0)),
            ("b", Handle::Nw, (-4.0, -4.0)),
            ("b", Handle::S, (0.0, 12.0)),
            ("r2", Handle::W, (-8.0, 0.0)),
        ] {
            let mut a = app_of(with_sized_b("fill"), on);
            let steps_before = steps(&a);
            let p = a
                .resize_preview(id, handle, pt(by.0, by.1), Modifiers::default())
                .unwrap();
            assert_eq!(steps(&a), steps_before, "a preview writes nothing");
            drag_handle(&mut a, id, handle, by);
            let (x, y, w, h) = box_of(&a, id);
            let (l, wd) = (p.local, p.world);
            assert!(
                (x - l.x).abs() < 1e-9 && (y - l.y).abs() < 1e-9,
                "{id} {handle:?} {on}"
            );
            assert!(
                (w - l.w).abs() < 1e-9 && (h - l.h).abs() < 1e-9,
                "{id} {handle:?} {on}"
            );
            // On the canvas: the box at the origin of its parent, which is the box the scene draws it at.
            assert!((wd.w - w).abs() < 1e-9 && (wd.h - h).abs() < 1e-9);
            let (ox, oy) = origin_of(&a, id);
            assert!((wd.x - (ox + x)).abs() < 1e-9 && (wd.y - (oy + y)).abs() < 1e-9);
        }
    }
}

#[test]
fn the_preview_of_a_flow_child_keeps_its_drawn_origin() {
    let a = app_of(with_sized_b("fill"), true);
    let p = a
        .resize_preview("b", Handle::Nw, pt(-4.0, -4.0), Modifiers::default())
        .unwrap();
    assert!(p.origin_fixed);
    assert_eq!((p.local.x, p.local.y), (10.0, 35.0));
    assert_eq!((p.local.w, p.local.h), (184.0, 34.0));
    // A free node moves its origin, with the layout on or off.
    for on in [false, true] {
        let a = app(on);
        let p = a
            .resize_preview("fg", Handle::Nw, pt(5.0, 7.0), Modifiers::default())
            .unwrap();
        assert!(!p.origin_fixed);
        assert_eq!((p.local.x, p.local.y), (25.0, 27.0));
    }
}

#[test]
fn a_node_in_a_frame_at_a_fractional_place_gets_the_stored_number_plus_the_drag_exactly() {
    // `fp` is at (1234.5678, 8765.4321) and `fc` is at (10.1, 20.3) in it: the difference of their world boxes is not 10.1.
    for on in [false, true] {
        for handle in [Handle::W, Handle::Nw, Handle::N, Handle::Ne, Handle::Sw] {
            let mut a = app_of(fractional(), on);
            let before = box_of(&a, "fc");
            if !on {
                assert_eq!((before.0, before.1), (10.1, 20.3));
            }
            drag_handle(&mut a, "fc", handle, (3.3, 1.7));
            let n = node(&a, "fc");
            let (west, north) = (
                matches!(handle, Handle::W | Handle::Nw | Handle::Sw),
                matches!(handle, Handle::N | Handle::Nw | Handle::Ne),
            );
            assert_eq!(
                n.x,
                if west { before.0 + 3.3 } else { 10.1 },
                "{handle:?} {on}"
            );
            assert_eq!(
                n.y,
                if north { before.1 + 1.7 } else { 20.3 },
                "{handle:?} {on}"
            );
            assert_eq!(
                n.w,
                if west {
                    before.2 - 3.3
                } else if matches!(handle, Handle::Ne) {
                    before.2 + 3.3
                } else {
                    before.2
                }
            );
        }
    }
}

#[test]
fn a_drag_that_changes_nothing_raises_no_event_and_one_that_does_raises_one_document_change() {
    let mut a = app(true);
    a.take_events();
    drag_handle(&mut a, "fg", Handle::E, (0.0, 0.0));
    assert!(a.take_events().is_empty());
    drag_handle(&mut a, "fg", Handle::E, (5.0, 0.0));
    assert_eq!(document_changes(&mut a), 1);
}

#[test]
fn a_preview_is_none_where_the_release_is_refused() {
    for on in [false, true] {
        let mut a = app(on);
        for by in [(2e6, 0.0), (0.0, 2e6), (f64::NAN, 0.0)] {
            let p = a.resize_preview("fg", Handle::Se, pt(by.0, by.1), Modifiers::default());
            drag_handle(&mut a, "fg", Handle::Se, by);
            assert!(p.is_none(), "{by:?} layout {on}");
        }
        // The west handle past the limit moves the origin out of the range.
        assert!(
            a.resize_preview("fg", Handle::W, pt(-2e6, 0.0), Modifiers::default())
                .is_none()
        );
        assert_eq!(refusals(&mut a), vec!["invalid"; 3]);
        assert_eq!(steps(&a), 0);
        // A drag that is allowed still has a preview.
        assert!(
            a.resize_preview("fg", Handle::Se, pt(5.0, 5.0), Modifiers::default())
                .is_some()
        );
    }
}
