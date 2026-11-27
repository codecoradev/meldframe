//! Property tests for the gestures of the canvas as intents (T8, decision 43): random draws and handle drags, with the layout
//! on and off, over a file with free nodes, flow children and an absolute child. Checked after every step:
//!
//! - no node of the file has a size that is not a number or is below 1, whatever the drag (long ones, flat ones, reversed ones
//!   and points that are not numbers included);
//! - a side the handle does not touch keeps its size, its origin and its sizing mode (a locked axis stays locked), and the
//!   origin of a flow child is never written;
//! - one gesture is one undo step at most, and one undo gives the file back exactly;
//! - a drag and its inverse drag (the same handle moved back) give the drawn box back, when the first one did not meet the
//!   smallest size;
//! - the preview is the box the release leaves;
//! - a node that was drawn is selected, the select tool is on again, and undoing it leaves the file as it was.
//!
//! Seeds per run: 20 by default; set `MELDFRAME_SEEDS` for more (the 600-seed runs stay native). `MELDFRAME_SEED=n` replays one
//! seed. A failure prints the seed and the step.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::{Action, App, CreateKind, Event, Handle, Modifiers, Tool};
use meldframe_protocol::{Node, SizeMode};
use serde_json::json;

const DEFAULT_SEEDS: u64 = 20;
const STEPS: usize = 40;
const HANDLES: [Handle; 8] = [
    Handle::Nw,
    Handle::N,
    Handle::Ne,
    Handle::E,
    Handle::Se,
    Handle::S,
    Handle::Sw,
    Handle::W,
];
const KINDS: [CreateKind; 4] = [
    CreateKind::Frame,
    CreateKind::Rectangle,
    CreateKind::Ellipse,
    CreateKind::Text,
];

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn seeds() -> std::ops::Range<u64> {
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(DEFAULT_SEEDS),
    }
}

/// A small deterministic generator, so a failing seed can be replayed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// A number in `[lo, hi)`, on a grid of quarters so that sums are exact.
    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        let steps = ((hi - lo) * 4.0) as usize;
        lo + self.below(steps.max(1)) as f64 / 4.0
    }
}

impl Rng {
    /// Any double in `[lo, hi)`: not on a grid, so that sums and differences round.
    fn real(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }
}

/// The edges a handle holds, `(west, east, north, south)`, from its name and not from `Handle::edges`, which the code under
/// test uses: `Nw` holds the north and the west edge.
fn sides(handle: Handle) -> (bool, bool, bool, bool) {
    let name = format!("{handle:?}").to_lowercase();
    (
        name.contains('w'),
        name.contains('e'),
        name.contains('n'),
        name.contains('s'),
    )
}

#[derive(Debug, Default)]
struct Stats {
    seeds: u64,
    drawn: u64,
    clicks: u64,
    resized: u64,
    flow_resized: u64,
    inverses: u64,
    refused: u64,
    far_draws: u64,
    exact: u64,
}

fn sizes_ok(a: &App, ctx: &str) {
    for n in a.nodes() {
        assert!(
            n.w.is_finite() && n.h.is_finite() && n.w >= 1.0 && n.h >= 1.0,
            "{ctx}: {} is {} x {}",
            n.id,
            n.w,
            n.h
        );
        assert!(n.x.is_finite() && n.y.is_finite(), "{ctx}: {} origin", n.id);
    }
}

fn find(a: &App, id: &str) -> Node {
    node(a, id)
}

fn run_seed(seed: u64, st: &mut Stats) {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let on = seed.is_multiple_of(2);
    let list = if seed % 4 < 2 {
        nodes()
    } else {
        with_sized_b("fill")
    };
    // A frame at a place that is not on a grid, with a rectangle at an offset that is not either.
    let mut list = list;
    list.extend(
        serde_json::from_value::<Vec<Node>>(json!([
            {"id": "fp", "type": "frame", "pos": "a9", "x": r.real(0.0, 5000.0), "y": r.real(2000.0, 9000.0), "w": 400, "h": 300},
            {"id": "fc", "type": "rect", "parentId": "fp", "pos": "a0", "x": r.real(0.0, 100.0), "y": r.real(0.0, 100.0),
             "w": r.real(20.0, 120.0), "h": r.real(20.0, 120.0)},
        ]))
        .unwrap(),
    );
    let mut a = app_of(list, on);
    let ids: Vec<String> = a.nodes().iter().map(|n| n.id.clone()).collect();
    for step in 0..STEPS {
        let ctx = format!("seed {seed} step {step} (layout {on})");
        let before = a.nodes().to_vec();
        let steps_before = steps(&a);
        if r.below(10) < 4 {
            draw_step(&mut a, &mut r, &ctx, st);
        } else {
            let id = ids[r.below(ids.len())].clone();
            resize_step(&mut a, &mut r, &id, &ctx, st);
        }
        sizes_ok(&a, &ctx);
        // One gesture is one undo step at most, and undoing what it did gives the file back exactly.
        let made = steps(&a) - steps_before;
        assert!(made <= 1, "{ctx}: {made} steps");
        for _ in 0..made {
            a.dispatch(Action::Undo {});
        }
        assert_eq!(snapshot(a.nodes()), snapshot(&before), "{ctx}: undo");
        a.take_events();
    }
}

fn draw_step(a: &mut App, r: &mut Rng, ctx: &str, st: &mut Stats) {
    let kind = KINDS[r.below(4)];
    // Now and then far past the limit of the file (a coordinate of 1,000,000): the release is refused.
    let far = r.below(10) == 0;
    let (lo, hi) = if far { (-3e6, 3e6) } else { (-50.0, 1100.0) };
    let from = (r.between(lo, hi), r.between(lo.min(-50.0), hi.min(400.0)));
    let to = match r.below(6) {
        0 => from,
        1 => (from.0 + r.between(-80.0, 80.0), from.1),
        2 => (from.0, from.1 + r.between(-80.0, 80.0)),
        _ => (r.between(lo, hi), r.between(lo.min(-50.0), hi.min(400.0))),
    };
    a.dispatch(Action::SetTool { tool: Tool::Frame });
    a.take_events();
    let before = a.nodes().to_vec();
    let preview = a.draw_preview(
        kind,
        pt(from.0, from.1),
        pt(to.0, to.1),
        Modifiers::default(),
    );
    let made = draw(a, kind, from, to);
    let events = a.take_events();
    // The preview answers none exactly where the release is refused.
    assert_eq!(preview.is_none(), made.is_none(), "{ctx}: {from:?} {to:?}");
    let (Some(preview), Some(made)) = (preview, made) else {
        assert_eq!(snapshot(a.nodes()), snapshot(&before), "{ctx}: refused");
        assert!(events.iter().any(|e| matches!(e, Event::Refused { .. })));
        st.refused += 1;
        st.far_draws += 1;
        return;
    };
    let n = find(a, &made);
    assert_eq!(n.parent_id, preview.parent, "{ctx}");
    assert_eq!(
        (n.x, n.y, n.w, n.h),
        (
            preview.local.x,
            preview.local.y,
            preview.local.w,
            preview.local.h
        ),
        "{ctx}"
    );
    // One gesture, one document change.
    let changes = events
        .iter()
        .filter(|e| matches!(e, Event::DocumentChanged { .. }))
        .count();
    assert_eq!(changes, 1, "{ctx}");
    assert_eq!(a.state().selection, vec![made.clone()], "{ctx}");
    assert_eq!(a.state().tool, Tool::Select, "{ctx}");
    assert_eq!(a.nodes().len(), before.len() + 1, "{ctx}");
    st.drawn += 1;
    st.clicks += u64::from(from == to);
}

fn resize_step(a: &mut App, r: &mut Rng, id: &str, ctx: &str, st: &mut Stats) {
    let handle = HANDLES[r.below(8)];
    let (west, east, north, south) = sides(handle);
    // Now and then a drag so long that the file refuses the size.
    let reach = [80.0, 80.0, 80.0, 80.0, 80.0, 5000.0, 1e8][r.below(7)];
    // On a grid of quarters (sums are exact) or any double (they round).
    let by = if r.below(2) == 0 {
        (r.between(-reach, reach), r.between(-reach, reach))
    } else {
        (r.real(-reach, reach), r.real(-reach, reach))
    };
    let before = a.nodes().to_vec();
    let was = find(a, id);
    let drawn = box_of(a, id);
    let preview = a.resize_preview(id, handle, pt(by.0, by.1), Modifiers::default());
    let steps_before = steps(a);
    a.take_events();
    drag_handle(a, id, handle, by);
    let events = a.take_events();
    let refused = events
        .iter()
        .filter(|e| matches!(e, Event::Refused { .. }))
        .count();
    let changes = events
        .iter()
        .filter(|e| matches!(e, Event::DocumentChanged { .. }))
        .count();
    st.refused += refused as u64;
    // The preview answers none exactly where the release is refused.
    assert_eq!(
        preview.is_none(),
        refused == 1,
        "{ctx}: {handle:?} by {by:?}"
    );
    let Some(preview) = preview else {
        assert_eq!(snapshot(a.nodes()), snapshot(&before), "{ctx}: refused");
        return;
    };
    // One gesture is one document change, and a drag that changes nothing raises no event at all.
    if steps(a) == steps_before {
        assert!(events.is_empty(), "{ctx}: a drag that changes nothing");
    } else {
        assert_eq!(changes, 1, "{ctx}");
    }
    let now = find(a, id);
    // A side the handle does not touch keeps its origin, its mode and the size it is drawn at. (The stored size of a hug or
    // fill side may be set to the drawn one, as `Action::Resize` always did: the mode ignores it.)
    let after = box_of(a, id);
    let near = |p: f64, q: f64| (p - q).abs() < 1e-6;
    if !(west || east) {
        assert_eq!(
            (now.x, now.sw),
            (was.x, was.sw),
            "{ctx}: {handle:?} holds the width"
        );
        assert!(
            near(after.2, drawn.2),
            "{ctx}: {handle:?} holds the drawn width"
        );
    }
    if !(north || south) {
        assert_eq!(
            (now.y, now.sh),
            (was.y, was.sh),
            "{ctx}: {handle:?} holds the height"
        );
        assert!(
            near(after.3, drawn.3),
            "{ctx}: {handle:?} holds the drawn height"
        );
    }
    // The origin of a flow child is the layout's: never written.
    if preview.origin_fixed {
        assert_eq!((now.x, now.y), (was.x, was.y), "{ctx}: a flow child");
        st.flow_resized += u64::from(steps(a) > steps_before);
    }
    if steps(a) == steps_before {
        assert_eq!(
            snapshot(a.nodes()),
            snapshot(&before),
            "{ctx}: no step, no change"
        );
        return;
    }
    st.resized += 1;
    let met = (east && drawn.2 + by.0 < 1.0)
        || (west && drawn.2 - by.0 < 1.0)
        || (south && drawn.3 + by.1 < 1.0)
        || (north && drawn.3 - by.1 < 1.0);
    // What is written is the box the scene draws plus the drag, to the last bit: no difference of two world boxes, and the
    // frame being at a place that is not a whole number changes nothing.
    if !met {
        if west {
            assert_eq!(now.w, drawn.2 - by.0, "{ctx}: {handle:?} width");
            assert!(preview.origin_fixed || now.x == drawn.0 + by.0, "{ctx}: x");
        }
        if east {
            assert_eq!(now.w, drawn.2 + by.0, "{ctx}: {handle:?} width");
        }
        if north {
            assert_eq!(now.h, drawn.3 - by.1, "{ctx}: {handle:?} height");
            assert!(preview.origin_fixed || now.y == drawn.1 + by.1, "{ctx}: y");
        }
        if south {
            assert_eq!(now.h, drawn.3 + by.1, "{ctx}: {handle:?} height");
        }
        st.exact += 1;
    }
    // The edges the handle does not hold stay where they are drawn, at any size (the position of a flow child is the
    // layout's, and its box is placed by the flow, so only the free ones are asked).
    if !preview.origin_fixed {
        let (right, bottom) = (drawn.0 + drawn.2, drawn.1 + drawn.3);
        assert!(
            !west || near(after.0 + after.2, right),
            "{ctx}: {handle:?} keeps the right edge"
        );
        assert!(
            !east || near(after.0, drawn.0),
            "{ctx}: {handle:?} keeps the left edge"
        );
        assert!(
            !north || near(after.1 + after.3, bottom),
            "{ctx}: {handle:?} keeps the bottom edge"
        );
        assert!(
            !south || near(after.1, drawn.1),
            "{ctx}: {handle:?} keeps the top edge"
        );
    }
    assert!(
        near(after.0, preview.local.x)
            && near(after.1, preview.local.y)
            && near(after.2, preview.local.w)
            && near(after.3, preview.local.h),
        "{ctx}: the preview {preview:?} is not the box {after:?}"
    );
    // Modes only ever go from hug or fill to fixed.
    assert!(
        matches!((was.sw, now.sw), (a, b) if a == b || b == SizeMode::Fixed),
        "{ctx}"
    );
    assert!(
        matches!((was.sh, now.sh), (a, b) if a == b || b == SizeMode::Fixed),
        "{ctx}"
    );
    // The same handle moved back gives the drawn box back, unless the drag met the smallest size.
    if !met && drawn.2 >= 1.0 && drawn.3 >= 1.0 {
        let applied = steps(a);
        drag_handle(a, id, handle, (-by.0, -by.1));
        a.take_events();
        let back = box_of(a, id);
        assert!(
            near(back.0, drawn.0)
                && near(back.1, drawn.1)
                && near(back.2, drawn.2)
                && near(back.3, drawn.3),
            "{ctx}: {handle:?} by {by:?} and back: {drawn:?} became {back:?}"
        );
        assert!(steps(a) <= applied + 1, "{ctx}");
        st.inverses += 1;
        // The two drags are two steps: take the second one back, the loop takes the first.
        if steps(a) > applied {
            a.dispatch(Action::Undo {});
        }
    }
}

#[test]
fn gestures_keep_sizes_axes_and_undo_under_random_drags() {
    let mut st = Stats::default();
    let range = seeds();
    let full = range.end - range.start >= 20;
    for seed in range {
        run_seed(seed, &mut st);
        st.seeds += 1;
    }
    println!("gestures_keep_sizes_axes_and_undo_under_random_drags: {st:?}");
    if full {
        // The run must not be idle.
        let n = st.seeds;
        assert!(st.drawn > 5 * n && st.clicks > n / 2, "{st:?}");
        assert!(st.resized > 10 * n && st.flow_resized > n / 2, "{st:?}");
        assert!(st.inverses > 5 * n && st.refused > n / 2, "{st:?}");
        assert!(st.far_draws > 0 && st.exact > 10 * n, "{st:?}");
    }
}

#[test]
fn a_drag_and_its_inverse_give_the_box_back_for_every_handle() {
    for on in [false, true] {
        for handle in HANDLES {
            let mut a = app(on);
            let before = box_of(&a, "fg");
            drag_handle(&mut a, "fg", handle, (7.0, -3.0));
            drag_handle(&mut a, "fg", handle, (-7.0, 3.0));
            assert_eq!(box_of(&a, "fg"), before, "{handle:?}, layout {on}");
        }
    }
}

#[test]
fn drawing_then_undoing_leaves_the_file_as_it_was() {
    for on in [false, true] {
        for kind in KINDS {
            let mut a = app_of(cards(), on);
            let before = a.nodes().to_vec();
            draw(&mut a, kind, (120.0, 70.0), (200.0, 120.0)).unwrap();
            a.dispatch(Action::Undo {});
            assert_eq!(snapshot(a.nodes()), snapshot(&before));
            assert!(a.state().selection.is_empty());
        }
    }
}
