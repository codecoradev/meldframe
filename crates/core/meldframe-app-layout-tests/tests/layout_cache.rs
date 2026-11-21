//! The app keeps the boxes of the last layout for the preview of a drop (and the drop and the resize that follow), so the
//! pointer moving over a file that has not changed lays nothing out again. It is keyed by the revision of the document
//! (the nodes with the pending ops) and by the measurer; these tests count the layouts (`App::layout_runs`) and check
//! that a cache hit and a fresh layout give the same answer after every kind of change.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::{Action, App, DropSlot, Frame, TextMeasurer};
use meldframe_protocol::{Command, Node, NodePatch};
use serde_json::json;

/// Every text measures to the same size: a new measurer is a visible change of the boxes.
struct Fixed(f64, f64);

impl TextMeasurer for Fixed {
    fn measure(&self, _: &Node, _: Option<f64>) -> (f64, f64) {
        (self.0, self.1)
    }
}

/// A column with two fixed rectangles and a hug text between them (so the measurer matters), and a second column.
fn list() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "f", "type": "frame", "pos": "a0", "w": 200, "h": 400, "layout": "column", "padding": 10, "gap": 5},
        {"id": "a", "type": "rect", "parentId": "f", "pos": "a0", "w": 50, "h": 20},
        {"id": "t", "type": "text", "parentId": "f", "pos": "a1", "text": "hello", "fontSize": 16, "sw": "hug", "sh": "hug"},
        {"id": "c", "type": "rect", "parentId": "f", "pos": "a2", "w": 50, "h": 40},
        {"id": "g", "type": "frame", "pos": "a1", "x": 400, "w": 200, "h": 200, "layout": "row"},
        {"id": "g1", "type": "rect", "parentId": "g", "pos": "a0", "w": 40, "h": 40},
    ]))
    .unwrap()
}

fn on() -> App {
    app_of(list(), true)
}

fn preview(a: &App, id: &str, at: (f64, f64)) -> Option<DropSlot> {
    a.drop_preview(id, at, (0.0, 0.0))
}

/// What a front end would ask while the pointer moves: a few points over both frames and off them.
const POINTS: [(f64, f64); 8] = [
    (30.0, 12.0),
    (30.0, 40.0),
    (30.0, 70.0),
    (30.0, 200.0),
    (420.0, 20.0),
    (430.0, 50.0),
    (700.0, 700.0),
    (-5.0, -5.0),
];

/// The previews of every node at every point, as text (a slot has no `Eq` on its line, and the text names the numbers).
fn answers(a: &App) -> Vec<String> {
    let ids: Vec<String> = a.nodes().iter().map(|n| n.id.clone()).collect();
    let mut out = Vec::new();
    for id in &ids {
        for p in POINTS {
            out.push(format!("{id}@{p:?}: {:?}", preview(a, id, p)));
        }
    }
    out
}

/// A fresh app on the same nodes with the same measurer: the answers the cache must equal.
fn fresh(a: &App, measurer: Option<(f64, f64)>) -> App {
    let mut b = App::new("user:me", "f0", 2, a.nodes().to_vec(), 0).unwrap();
    b.set_layout_on_canvas(true);
    if let Some((w, h)) = measurer {
        b.set_measurer(Box::new(Fixed(w, h)));
    }
    b
}

#[test]
fn a_second_preview_of_a_file_that_has_not_changed_does_not_lay_it_out_again() {
    let a = on();
    assert_eq!(a.layout_runs(), 0, "nothing is laid out until it is asked");
    preview(&a, "a", POINTS[0]);
    assert_eq!(a.layout_runs(), 1);
    // The same point, and every other: the pointer moves, the file does not.
    preview(&a, "a", POINTS[0]);
    for p in POINTS {
        preview(&a, "c", p);
    }
    assert_eq!(a.layout_runs(), 1, "one layout for all of them");
}

#[test]
fn the_drop_after_the_previews_reads_the_same_layout_and_the_next_preview_lays_out_the_new_file() {
    let mut a = on();
    preview(&a, "a", (30.0, 200.0));
    assert_eq!(a.layout_runs(), 1);
    let sent = drop_node(&mut a, "a", (30.0, 200.0), (0.0, 0.0));
    assert!(!sent.is_empty(), "the node moved in its flow");
    assert_eq!(
        a.layout_runs(),
        1,
        "the drop read the boxes the preview had worked out"
    );
    preview(&a, "a", (30.0, 12.0));
    assert_eq!(
        a.layout_runs(),
        2,
        "the document changed, so one more layout"
    );
}

#[test]
fn a_resize_reads_the_same_layout() {
    let mut a = on();
    preview(&a, "a", POINTS[0]);
    a.dispatch(Action::Resize {
        id: "a".into(),
        w: 60.0,
        h: 20.0,
        x: None,
        y: None,
    });
    assert_eq!(a.layout_runs(), 1, "the resize used the cached boxes");
}

#[test]
fn nothing_that_leaves_the_boxes_as_they_are_lays_the_file_out_again() {
    let mut a = on();
    preview(&a, "a", POINTS[0]);
    // The selection, the tool, the panels, the mode of a collection and the flag are not the document.
    a.dispatch(Action::Select {
        ids: vec!["a".into()],
    });
    a.dispatch(Action::SetTool {
        tool: meldframe_app::Tool::Frame,
    });
    a.dispatch(Action::CreateCollection {
        name: "brand".into(),
    });
    let created = a.layout_runs();
    preview(&a, "a", POINTS[0]);
    // The collection is a change of the document (its own revision), so the count may have gone up by one: what must
    // hold is that a pick of a mode, a toggle of the flag and a tick do not.
    let doc_runs = a.layout_runs();
    assert!(doc_runs <= created + 1);
    a.dispatch(Action::SetMode {
        collection: a.vars().collections[0].id.clone(),
        mode: None,
    });
    a.set_layout_on_canvas(false);
    a.set_layout_on_canvas(true);
    a.dispatch(Action::Tick { now_ms: 5_000 });
    preview(&a, "a", POINTS[0]);
    assert_eq!(a.layout_runs(), doc_runs, "no box changed, so no layout");
}

#[test]
fn every_change_of_the_document_lays_out_once_and_the_cache_agrees_with_a_fresh_layout() {
    let mut a = on();
    assert_eq!(answers(&a), answers(&fresh(&a, None)));
    let mut runs = a.layout_runs();
    let mut check = |a: &mut App, what: &str| {
        let got = answers(a);
        assert_eq!(a.layout_runs(), runs + 1, "{what}: one layout, then cached");
        assert_eq!(got, answers(&fresh(a, None)), "{what}: the cache is stale");
        assert_eq!(answers(a), got, "{what}: asked again");
        runs = a.layout_runs();
    };
    // An edit of mine that changes a box.
    a.dispatch(Action::Resize {
        id: "c".into(),
        w: 50.0,
        h: 90.0,
        x: None,
        y: None,
    });
    check(&mut a, "a resize");
    // A drop that reorders.
    drop_node(&mut a, "a", (30.0, 300.0), (0.0, 0.0));
    check(&mut a, "a drop");
    // Undo and redo.
    a.dispatch(Action::Undo {});
    check(&mut a, "an undo");
    a.dispatch(Action::Redo {});
    check(&mut a, "a redo");
    // Another actor's op, ordered after mine.
    a.dispatch(Action::Frame {
        now_ms: 1,
        frame: Frame::Event {
            seq: 1,
            actor: "agent:other".into(),
            key: None,
            command: Command::UpdateNode {
                id: "a".into(),
                patch: NodePatch {
                    h: Some(100.0),
                    ..Default::default()
                },
            },
        },
    });
    check(&mut a, "an event of another actor");
    // The file loaded again (a reload replaces the view as a whole).
    let reloaded = list();
    a.dispatch(Action::Reload {
        now_ms: 2,
        nodes: reloaded,
        variables: Default::default(),
        seq: 5,
        applied: vec![],
    });
    check(&mut a, "a reload");
}

#[test]
fn a_new_measurer_lays_out_again_even_the_same_one_given_afresh() {
    let mut a = on();
    let before = answers(&a);
    a.set_measurer(Box::new(Fixed(77.0, 33.0)));
    let after = answers(&a);
    assert_eq!(a.layout_runs(), 2, "the stub, then the new measurer");
    assert_ne!(after, before, "the hug text has another size");
    assert_eq!(after, answers(&fresh(&a, Some((77.0, 33.0)))));
    // The host gives a measurer again when its fonts load: its answers may differ, so the boxes are worked out again.
    a.set_measurer(Box::new(Fixed(77.0, 33.0)));
    answers(&a);
    assert_eq!(a.layout_runs(), 3);
}

#[test]
fn with_the_flag_off_nothing_is_laid_out_for_a_drop_or_a_preview() {
    let mut a = app_of(list(), false);
    assert!(preview(&a, "a", POINTS[0]).is_none());
    drop_node(&mut a, "a", (30.0, 12.0), (3.0, 4.0));
    assert_eq!(a.layout_runs(), 0);
}

#[test]
fn a_resync_gives_new_boxes_even_when_the_view_version_matches_the_old_one() {
    // A reload builds a new view whose own version starts again: the old boxes must not be kept for it. Both files are
    // loaded the same way (no edit since), so the version of their views is the same number; only the epoch tells them apart.
    let mut a = on();
    let before = answers(&a);
    let at = (30.0, 12.0);
    let old = preview(&a, "a", at).expect("a slot in the old file");
    assert_eq!(a.layout_runs(), 1);
    // The same ids and the same flow, but the frame is a row and the children are far apart: another place for the same point.
    let other: Vec<Node> = serde_json::from_value(json!([
        {"id": "f", "type": "frame", "pos": "a0", "w": 600, "h": 400, "layout": "row", "padding": 10, "gap": 100},
        {"id": "a", "type": "rect", "parentId": "f", "pos": "a0", "w": 50, "h": 20},
        {"id": "t", "type": "text", "parentId": "f", "pos": "a1", "text": "hello", "fontSize": 16, "sw": "hug", "sh": "hug"},
        {"id": "c", "type": "rect", "parentId": "f", "pos": "a2", "w": 50, "h": 40},
        {"id": "g", "type": "frame", "pos": "a1", "x": 700, "w": 200, "h": 200, "layout": "row"},
        {"id": "g1", "type": "rect", "parentId": "g", "pos": "a0", "w": 40, "h": 40},
    ]))
    .unwrap();
    a.dispatch(Action::Reload {
        now_ms: 1,
        nodes: other.clone(),
        variables: Default::default(),
        seq: 3,
        applied: vec![],
    });
    let new = preview(&a, "a", (130.0, 12.0));
    assert_eq!(
        a.layout_runs(),
        2,
        "the reloaded file is laid out, not read from the cache of the old one"
    );
    let mut fresh = App::new("user:me", "f0", 2, other, 3).unwrap();
    fresh.set_layout_on_canvas(true);
    assert_eq!(
        format!("{new:?}"),
        format!("{:?}", preview(&fresh, "a", (130.0, 12.0)))
    );
    assert_eq!(answers(&a), answers(&fresh));
    assert_ne!(answers(&a), before);
    assert_ne!(format!("{new:?}"), format!("{old:?}"));
}
