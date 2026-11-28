//! C5 Hit (section 8 of the spike note), the call part: `App::hit` in this process, native, over the files of
//! `tools/hit-perf.mjs` with the same seed. The click part (C5b) needs a window and is not run here.
//!
//! The numbers are those of the machine this runs on, not of the Mac of the run record; a debug build is not judged.

use crate::files::{hit_file, lcg, over_a_box, over_nothing};
use crate::report::Report;
use crate::stats::{max, mean, ms, percentile};
use meldframe_app::{Action, App};

/// A quarter of a frame at 60 Hz [E].
pub const BUDGET_MS: f64 = 16.7 / 4.0;

/// How a row picks its next point from the shared random sequence.
type Picker<'a> = &'a mut dyn FnMut(&mut dyn FnMut() -> f64) -> (f64, f64);

/// Runs C5a for each size in `sizes`, `calls` calls each row.
pub fn run(sizes: &[usize], calls: usize) -> Report {
    let release = !cfg!(debug_assertions);
    let mut r = Report::new("C5");
    r.label("profile", if release { "release" } else { "debug" })
        .label("path", "native_process_no_bridge");
    let mut over = false;
    for &n in sizes {
        let (nodes, groups) = hit_file(n);
        let count = nodes.len();
        for layout in [true, false] {
            let mut app =
                App::new("user:measure", "measure", 1, nodes.clone(), 0).expect("the file loads");
            app.set_layout_on_canvas(layout);
            let first = ms(|| {
                let _ = app.hit(5.0, 5.0);
            });
            let mut rand = lcg(7);
            let mut run = |point: Picker| {
                (0..calls)
                    .map(|_| {
                        let (x, y) = point(&mut rand);
                        ms(|| {
                            let _ = app.hit(x, y);
                        })
                    })
                    .collect::<Vec<f64>>()
            };
            let hit = run(&mut |rand| over_a_box(rand, groups));
            let miss = run(&mut |rand| over_nothing(rand));
            // An edit changes the revision: the next call builds the scene and the index again.
            app.dispatch(Action::Move {
                id: "r0_0".into(),
                x: 20.0,
                y: 20.0,
            });
            let rebuild = ms(|| {
                let _ = app.hit(5.0, 5.0);
            });
            let (nodes_s, layout_s, calls_s) = (
                count.to_string(),
                if layout { "on" } else { "off" },
                calls.to_string(),
            );
            let l = [
                ("nodes", nodes_s.as_str()),
                ("layout", layout_s),
                ("calls", calls_s.as_str()),
            ];
            r.num("c5a.first_call_ms", first, "ms", &l)
                .num("c5a.hit_mean_ms", mean(&hit), "ms", &l)
                .num("c5a.hit_p99_ms", percentile(&hit, 0.99), "ms", &l)
                .num("c5a.hit_max_ms", max(&hit), "ms", &l)
                .num("c5a.miss_mean_ms", mean(&miss), "ms", &l)
                .num("c5a.miss_p99_ms", percentile(&miss, 0.99), "ms", &l)
                .num("c5a.miss_max_ms", max(&miss), "ms", &l)
                .num("c5a.rebuild_first_call_ms", rebuild, "ms", &l);
            over |= percentile(&hit, 0.99) > BUDGET_MS || percentile(&miss, 0.99) > BUDGET_MS;
        }
    }
    r.num("c5a.budget_p99_ms", BUDGET_MS, "ms", &[]).word(
        "c5b.status",
        "skipped",
        &[(
            "reason",
            "needs_a_window_and_a_pointer_event_loop_a_real_gpu_path",
        )],
    );
    let c5a = match (release, over) {
        (false, _) => "open",
        (true, true) => "gap",
        (true, false) => "green",
    };
    r.word(
        "c5a.verdict",
        c5a,
        &[
            ("machine", "this_run_only"),
            ("rows_judged", "hit_and_miss_not_rebuild"),
        ],
    )
    .status(
        "partial",
        "C5a ran in this process; C5b (the click in a real window) did not",
    )
    .verdict("open");
    r
}
