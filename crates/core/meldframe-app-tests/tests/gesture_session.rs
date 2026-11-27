//! A person draws a frame, draws a rectangle in it, sizes the rectangle by a handle and undoes both, with no front end: the
//! intents are world points and the answers are read from the state of the app (T8, decision 43), through the real gate
//! and room, and a second editor sees what the first one did.

use meldframe_app::{Action, CreateKind, GestureOp, Handle, Modifiers, Point, Tool};
use meldframe_app_harness::*;

fn draw(kind: CreateKind, from: (f64, f64), to: (f64, f64)) -> Action {
    Action::Gesture {
        op: GestureOp::Draw {
            kind,
            from: Point {
                x: from.0,
                y: from.1,
            },
            to: Point { x: to.0, y: to.1 },
            mods: Modifiers::default(),
        },
    }
}

fn size_by(id: &str, handle: Handle, by: (f64, f64)) -> Action {
    Action::Gesture {
        op: GestureOp::ResizeHandle {
            id: id.into(),
            handle,
            by: Point { x: by.0, y: by.1 },
            mods: Modifiers::default(),
        },
    }
}

fn box_of(w: &World, i: usize, id: &str) -> Option<(Option<String>, f64, f64, f64, f64)> {
    let n = w.app(i).nodes().iter().find(|n| n.id == id)?;
    Some((n.parent_id.clone(), n.x, n.y, n.w, n.h))
}

#[test]
fn draw_a_frame_draw_a_rectangle_in_it_resize_the_rectangle_and_undo_both() {
    let mut w = World::new(64);
    let me = w.join("user:me", "a", 1);
    let other = w.join("user:other", "b", 1);
    let start = w.app(me).nodes().len();

    // A frame on the empty canvas, then a rectangle inside it: its place is in the frame's space.
    w.act(me, Action::SetTool { tool: Tool::Frame });
    w.act(
        me,
        draw(CreateKind::Frame, (2000.0, 2000.0), (2400.0, 2300.0)),
    );
    let frame = w.app(me).state().selection[0].clone();
    assert_eq!(
        box_of(&w, me, &frame),
        Some((None, 2000.0, 2000.0, 400.0, 300.0))
    );
    assert_eq!(w.app(me).state().tool, Tool::Select);
    w.act(
        me,
        draw(CreateKind::Rectangle, (2050.0, 2060.0), (2150.0, 2110.0)),
    );
    let rect = w.app(me).state().selection[0].clone();
    assert_eq!(
        box_of(&w, me, &rect),
        Some((Some(frame.clone()), 50.0, 60.0, 100.0, 50.0))
    );

    // The south east handle by 50 and 40: the north west corner stays.
    w.act(me, size_by(&rect, Handle::Se, (50.0, 40.0)));
    assert_eq!(
        box_of(&w, me, &rect),
        Some((Some(frame.clone()), 50.0, 60.0, 150.0, 90.0))
    );
    assert_eq!(w.app(me).state().history.undo_steps, 3);

    // The other editor sees the same file once the ops went through the room.
    w.pump_all();
    assert_eq!(
        box_of(&w, other, &rect),
        Some((Some(frame.clone()), 50.0, 60.0, 150.0, 90.0))
    );

    // One undo takes the resize back, the next takes the rectangle away; the frame stays.
    w.act(me, Action::Undo {});
    assert_eq!(
        box_of(&w, me, &rect),
        Some((Some(frame.clone()), 50.0, 60.0, 100.0, 50.0))
    );
    w.act(me, Action::Undo {});
    assert_eq!(box_of(&w, me, &rect), None);
    assert!(box_of(&w, me, &frame).is_some());
    assert_eq!(w.app(me).nodes().len(), start + 1);
    w.pump_all();
    assert_eq!(box_of(&w, other, &rect), None);
    w.assert_converged();
}
