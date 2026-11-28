//! C4 Frame (section 7 of the spike note). The criterion is judged on a real GPU path in a real window with input pumped
//! continuously, which this run does not have. What runs here is labelled for what it is and never judged against the budget:
//!
//! - the CPU cost of a tick of the headless runtime (a camera pan sent to the app layer, then one frame: the view and the
//!   instances, nothing presented), alone and with the 4x throttle of section 7 (three times the CPU time spun after each frame);
//! - the cost of working the scene out and building the shape once (the cost of an edit);
//! - when the run is on a software adapter and the build is a debug one, the time of one headless render (`--snap`, which a
//!   release build has only with Rune's `tooling` feature), start-up included.
//!
//! The painter ignores the camera (it draws the home view only), so a pan changes no scene and no shape: the tick cost measured
//! here is the cost of the shell's view with a cached shape, which a real pan will exceed.

use crate::env::software_adapter;
use crate::files::hit_file;
use crate::headless;
use crate::report::Report;
use crate::stats::{max, ms, percentile};
use meldframe_app::{Action, App, CameraOp};
use std::time::Instant;

/// The limit of one frame [E]: no interval over 20 ms, p95 at most 18 ms.
pub const FRAME_LIMIT_MS: f64 = 20.0;

/// A tick: one pan step sent to the app layer and one frame of the runtime. With a throttle, the CPU time is spun again that
/// many times less one. Returns the CPU time of the tick in milliseconds as the criterion would see it.
fn ticks(n: usize, count: usize, throttle: u32) -> (Vec<f64>, u64, u64) {
    let (nodes, _) = hit_file(n);
    let mut rt = headless::runtime(nodes);
    let (builds0, resolves0) = (rt.state.shape().builds(), rt.state.app().scene_resolves());
    let mut cpu = Vec::with_capacity(count);
    for k in 0..count {
        // Back and forth by a quarter of the window width over 60 ticks, as a drag at 60 Hz would move it.
        let dx = if (k / 30) % 2 == 0 { 7.5 } else { -7.5 };
        let t = Instant::now();
        rt.update(|s, _| {
            s.act(Action::Camera {
                op: CameraOp::Pan { dx, dy: 0.0 },
            })
        });
        rt.frame(1.0 / 60.0);
        let one = t.elapsed();
        if throttle > 1 {
            let until = Instant::now() + one * (throttle - 1);
            while Instant::now() < until {
                std::hint::spin_loop();
            }
        }
        cpu.push(one.as_secs_f64() * 1e3 * f64::from(throttle.max(1)));
    }
    let builds = rt.state.shape().builds() - builds0;
    let resolves = rt.state.app().scene_resolves() - resolves0;
    (cpu, builds, resolves)
}

/// Runs what C4 can run without a real GPU: `sizes` nodes, `count` ticks each.
pub fn run(sizes: &[usize], count: usize, snap_dir: &std::path::Path) -> Report {
    let software = software_adapter();
    let mut r = Report::new("C4");
    r.label("kind", "cpu_only_headless_runtime")
        .label("judged", "no")
        .label(
            "profile",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        );
    for &n in sizes {
        let (nodes, _) = hit_file(n);
        let total = nodes.len().to_string();
        let l = [("nodes", total.as_str())];
        let mut app =
            App::new("user:measure", "measure", 1, nodes.clone(), 0).expect("the file loads");
        app.set_layout_on_canvas(true);
        let build = ms(|| {
            let _ = app.with_scene(|_, boxes| canvas_ui_rune::paint(boxes).ops.len());
        });
        r.num("c4.scene_and_shape_build_ms", build, "ms", &l);
        for throttle in [1u32, 4] {
            let (cpu, builds, resolves) = ticks(n, count, throttle);
            let t = [
                ("nodes", total.as_str()),
                ("throttle", if throttle == 1 { "none" } else { "4x" }),
            ];
            r.int("c4.ticks", cpu.len() as i64, "count", &t)
                .num("c4.tick_cpu_p50_ms", percentile(&cpu, 0.5), "ms", &t)
                .num("c4.tick_cpu_p95_ms", percentile(&cpu, 0.95), "ms", &t)
                .num("c4.tick_cpu_p99_ms", percentile(&cpu, 0.99), "ms", &t)
                .num("c4.tick_cpu_max_ms", max(&cpu), "ms", &t)
                .int(
                    "c4.ticks_over_20ms",
                    cpu.iter().filter(|v| **v > FRAME_LIMIT_MS).count() as i64,
                    "count",
                    &t,
                )
                .int("c4.shape_builds_during_ticks", builds as i64, "count", &t)
                .int(
                    "c4.scene_resolves_during_ticks",
                    resolves as i64,
                    "count",
                    &t,
                );
        }
    }
    if software && cfg!(debug_assertions) {
        let (nodes, _) = hit_file(sizes.first().copied().unwrap_or(5000));
        let total = nodes.len().to_string();
        let out = snap_dir.join("c4-snap.png");
        let _ = std::fs::create_dir_all(snap_dir);
        for k in 0..3 {
            match headless::snap(nodes.clone(), &out) {
                Ok(t) => {
                    r.num(
                        "c4.software_adapter_snap_ms",
                        t,
                        "ms",
                        &[
                            ("nodes", total.as_str()),
                            ("run", &k.to_string()),
                            ("adapter", "software"),
                        ],
                    );
                }
                Err(e) => {
                    r.word("c4.software_adapter_snap", "failed", &[("error", &e)]);
                    break;
                }
            }
        }
    } else {
        // `--snap` is Rune agent tooling: a debug build always has it and a release build has it only with Rune's `tooling`
        // feature, which this workspace does not turn on (it would change the binary C10 measures).
        let reason = if software {
            "release_build_has_no_Rune_tooling_feature_run_the_debug_build_for_the_snap"
        } else {
            "no_software_adapter_named_by_VK_ICD_FILENAMES"
        };
        r.word("c4.snap", "skipped", &[("reason", reason)]);
    }
    r.word("c4.real_gpu_frame_interval", "skipped", &[("reason", "needs_a_physical_machine_with_a_display_and_a_real_gpu_path")])
        .status("partial", "only the CPU cost of a headless tick ran; the frame interval on a real GPU path is the criterion")
        .verdict("open");
    r
}
