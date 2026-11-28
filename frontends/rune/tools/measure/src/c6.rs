//! C6 Idle (section 9 of the spike note). The criterion is a real window with no input for 180 s. What runs here is the same
//! shell on Rune's headless runtime, driven at 60 Hz in real time with no input at all after the first frame, reading how many
//! frames the runtime asks for and the resident memory of the process once a second. It has no window, no GPU and no socket,
//! so it can show a redraw the runtime asks for by itself, and cannot show one that a window, a swap chain or a connection adds.
//!
//! The memory rule of the note [P] (within 2 percent between 30 s and the end, a slope of at most 10 KiB per second) is printed as the
//! two numbers it reads and is not applied to this run: a headless run is not the criterion.
//!
//! The frames are counted by `rune::commit_generation()`, the number of runs of a view that go on to the screen; the answer of
//! `Runtime::frame` is counted too and is not trusted alone, because the control (one input) does not make it true. A control
//! that moves no counter would make the idle counts mean nothing, and the report says so.

use crate::files::hit_file;
use crate::headless;
use crate::report::Report;
use crate::stats::slope;
use std::time::{Duration, Instant};

/// The resident set of this process in KiB: `/proc/self/status` on Linux, `ps` elsewhere.
pub fn resident_kib() -> Option<f64> {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        return status
            .lines()
            .find_map(|l| l.strip_prefix("VmRSS:"))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok());
    }
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Runs the idle loop for `seconds` on a file of about `n` nodes.
pub fn run(n: usize, seconds: u64) -> Report {
    let (nodes, _) = hit_file(n);
    let total = nodes.len().to_string();
    let mut r = Report::new("C6");
    r.label("kind", "headless_runtime_no_window_no_gpu")
        .label("nodes", &total)
        .label("judged", "no");
    let mut rt = headless::runtime(nodes);
    let (mut asked, mut first_minute, mut commits_first_minute) = (0i64, 0i64, 0i64);
    let commits_at_rest = rune::commit_generation();
    let mut series: Vec<(f64, f64)> = Vec::new();
    let start = Instant::now();
    let mut next_sample = 0.0;
    let tick = Duration::from_secs_f64(1.0 / 60.0);
    let mut due = start;
    while start.elapsed().as_secs() < seconds {
        let at = start.elapsed().as_secs_f64();
        if at >= next_sample {
            if let Some(kib) = resident_kib() {
                series.push((at, kib));
            }
            next_sample += 1.0;
        }
        if rt.frame(1.0 / 60.0) {
            asked += 1;
            if at < 60.0 {
                first_minute += 1;
            }
        }
        if at < 60.0 {
            commits_first_minute = (rune::commit_generation() - commits_at_rest) as i64;
        }
        due += tick;
        std::thread::sleep(due.saturating_duration_since(Instant::now()));
    }
    let commits_idle = (rune::commit_generation() - commits_at_rest) as i64;
    // The control: one input must move the commit counter, or its silence at rest proves nothing.
    rt.update(|s, _| {
        s.act(meldframe_app::Action::Camera {
            op: meldframe_app::CameraOp::Pan { dx: 5.0, dy: 0.0 },
        });
    });
    let before = rune::commit_generation();
    let after_input = rt.frame(1.0 / 60.0);
    let control = (rune::commit_generation() - before) as i64;
    r.int(
        "c6.control_commits_after_one_input",
        control,
        "count",
        &[
            ("meaning", "if_0_the_idle_counts_say_nothing"),
            ("frame_asked", if after_input { "yes" } else { "no" }),
        ],
    );
    r.int("c6.seconds", seconds as i64, "s", &[])
        .int(
            "c6.view_commits",
            commits_idle,
            "count",
            &[("window", "whole_run_after_first_paint")],
        )
        .int(
            "c6.view_commits_first_60s",
            commits_first_minute,
            "count",
            &[("window", "0_to_60s")],
        )
        .int(
            "c6.frame_asks",
            asked,
            "count",
            &[
                ("window", "whole_run"),
                ("meaning", "Runtime::frame_returned_true"),
            ],
        )
        .int(
            "c6.frame_asks_first_60s",
            first_minute,
            "count",
            &[("window", "0_to_60s")],
        );
    let at = |t: f64| series.iter().find(|(s, _)| *s >= t).map(|(_, k)| *k);
    if let (Some(first), Some(last)) = (series.first(), series.last()) {
        r.num("c6.rss_first_kib", first.1, "KiB", &[]).num(
            "c6.rss_last_kib",
            last.1,
            "KiB",
            &[("at_s", &format!("{:.0}", last.0))],
        );
        let tail: Vec<(f64, f64)> = series.iter().copied().filter(|(s, _)| *s >= 30.0).collect();
        if let Some(at30) = at(30.0).filter(|_| seconds >= 180) {
            r.num("c6.rss_30s_kib", at30, "KiB", &[])
                .num(
                    "c6.rss_change_30s_to_end_percent",
                    (last.1 - at30) / at30 * 100.0,
                    "percent",
                    &[],
                )
                .num(
                    "c6.rss_slope_30s_to_end_kib_per_s",
                    slope(&tail),
                    "KiB/s",
                    &[],
                );
        } else {
            r.word(
                "c6.rss_flat",
                "not_judged",
                &[("reason", "the_run_is_shorter_than_180s")],
            );
        }
    } else {
        r.word(
            "c6.rss",
            "not_read",
            &[("reason", "the_resident_set_cannot_be_read_here")],
        );
    }
    r.word(
        "c6.window_frames",
        "skipped",
        &[(
            "reason",
            "needs_a_real_window_a_swap_chain_and_a_server_connection",
        )],
    )
    .status(
        "partial",
        "headless runtime only: no window, no GPU, no socket",
    )
    .verdict("open");
    r
}
