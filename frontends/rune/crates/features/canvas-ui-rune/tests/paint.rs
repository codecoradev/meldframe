//! The painter and the cache of the canvas, with no window: what a scene is painted as (placement, alpha, the cases at the
//! edge) and what a view costs when the scene has not changed.

// A counting allocator, to see whether a view copies the shape. The counters belong to a thread, so the tests of this file do
// not count each other's allocations.
#![allow(unsafe_code)]

use canvas_ui_rune::{SceneShape, paint, view};
use meldframe_app::{Action, App};
use meldframe_protocol::Node;
use meldframe_render::PaintedBox;
use rune::shape::Op;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static BYTES: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: every call is handed to the system allocator unchanged; the counter is a plain thread local `Cell`.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        BYTES.with(|b| b.set(b.get() + layout.size()));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The bytes this thread asked the allocator for while `f` ran.
fn allocated(f: impl FnOnce()) -> usize {
    let before = BYTES.with(Cell::get);
    f();
    BYTES.with(Cell::get) - before
}

fn app_of(nodes: &str) -> App {
    let nodes: Vec<Node> = serde_json::from_str(nodes).expect("nodes parse");
    App::new("user:test", "test", 1, nodes, 0).expect("loads")
}

fn many(count: usize) -> App {
    let mut nodes = vec![
        r#"{"id": "root", "type": "frame", "x": 0, "y": 0, "w": 9000, "h": 9000}"#.to_string(),
    ];
    for i in 0..count {
        nodes.push(
            include_str!("scenes/leaf.json")
                .trim()
                .replace("{i}", &i.to_string())
                .replace("{x}", &((i % 90) * 10).to_string())
                .replace("{y}", &((i / 90) * 10).to_string()),
        );
    }
    app_of(&format!("[{}]", nodes.join(",")))
}

/// The alpha of each fill of a scene, back to front, and the corners `[x0, y0, x1, y1]` it covers.
fn fills(boxes: &[PaintedBox]) -> Vec<(f32, [f32; 4])> {
    paint(boxes)
        .ops
        .iter()
        .filter_map(|op| {
            let (points, a): (Vec<[f32; 2]>, f32) = match op {
                Op::Fill { points, color } => (points.clone(), color.a),
                Op::FillCompound {
                    contours, color, ..
                } => (contours.concat(), color.a),
                _ => return None,
            };
            let lo = |i: usize| points.iter().map(|p| p[i]).fold(f32::MAX, f32::min);
            let hi = |i: usize| points.iter().map(|p| p[i]).fold(f32::MIN, f32::max);
            Some((a, [lo(0), lo(1), hi(0), hi(1)]))
        })
        .collect()
}

fn scene(app: &App) -> Vec<PaintedBox> {
    app.scene()
}

#[test]
fn a_view_with_an_unchanged_epoch_copies_no_shape() {
    let app = many(2000);
    let shape = SceneShape::default();
    let first = allocated(|| drop(view::<()>(&app, &shape, |_, _| {})));
    let again = allocated(|| drop(view::<()>(&app, &shape, |_, _| {})));
    assert_eq!(shape.builds(), 1);
    eprintln!(
        "a view of 2001 boxes allocated {first} bytes when it built the shape and {again} when it did not"
    );
    // The first view builds the shape (about a hundred bytes a box and more); the second hands the held element on.
    assert!(
        first > 200_000,
        "the first view builds the shape: {first} bytes"
    );
    assert!(
        again < 8_000,
        "a view that finds the epoch unchanged allocated {again} bytes, the shape is being copied"
    );
}

#[test]
fn an_edit_builds_the_shape_once_more_and_nothing_else_does() {
    let mut app = many(10);
    let shape = SceneShape::default();
    drop(view::<()>(&app, &shape, |_, _| {}));
    app.dispatch(Action::Select {
        ids: vec!["r1".into()],
    });
    drop(view::<()>(&app, &shape, |_, _| {}));
    assert_eq!(shape.builds(), 1, "a selection is no scene change");
    app.dispatch(Action::MoveBy { dx: 5.0, dy: 0.0 });
    drop(view::<()>(&app, &shape, |_, _| {}));
    assert_eq!(shape.builds(), 2);
    assert_eq!(shape.epoch(), Some(app.scene_epoch()));
}

#[test]
fn a_box_under_a_faded_frame_is_painted_as_faded_as_the_frame_and_itself() {
    let app = app_of(include_str!("scenes/faded_frame.json"));
    let alphas: Vec<f32> = fills(&scene(&app)).iter().map(|f| f.0).collect();
    // The page and each box below it multiply the opacity of the boxes above: 0.5, 0.25, 0.25, and a box in no frame is 1.
    // The page has no parent either, so the fourth is the page's sibling at full opacity.
    assert_eq!(alphas, [0.5, 0.25, 0.25, 1.0]);
}

#[test]
fn a_repeated_id_is_painted_once_at_the_place_of_the_first() {
    let app = app_of(include_str!("scenes/repeated_id.json"));
    let mut boxes = scene(&app);
    let mut again = boxes[1].clone();
    again.x = 300.0;
    boxes.push(again);
    let covered: Vec<[f32; 4]> = fills(&boxes).iter().map(|f| f.1).collect();
    assert_eq!(
        covered,
        [[0.0, 0.0, 400.0, 400.0], [10.0, 10.0, 60.0, 50.0]],
        "the index never finds the second box, so it is not drawn"
    );
}

#[test]
fn an_empty_scene_paints_nothing_and_a_view_of_it_works() {
    assert!(paint(&[]).is_empty());
    let app = app_of("[]");
    let shape = SceneShape::default();
    drop(view::<()>(&app, &shape, |_, _| {}));
    assert_eq!(shape.builds(), 1);
}

#[test]
fn a_box_with_no_size_and_a_box_at_a_negative_place_do_not_break_the_others() {
    let app = app_of(include_str!("scenes/edge_cases.json"));
    let covered: Vec<[f32; 4]> = fills(&scene(&app)).iter().map(|f| f.1).collect();
    assert!(
        covered.contains(&[-30.0, -5.0, 20.0, 35.0]),
        "the box at a negative place: {covered:?}"
    );
    assert!(covered.contains(&[0.0, 0.0, 400.0, 400.0]));
    assert!(covered.iter().all(|c| c.iter().all(|v| v.is_finite())));
}
