//! Property tests for the replica: random runs of several replicas against an in-memory server, with
//! the dependency rule, delayed delivery (random order across replicas), repeated delivery of ops to the
//! server and of events to the replicas, and refused ops. Not modelled yet: a lost or reordered event on
//! the way to a replica, which needs the resync entry point (issue 90).
//!
//! Checked on every seed (I1, convergence): when the queues are empty every replica equals the server,
//! nothing is pending, the view is `base + pending` after every event, and sibling order is valid and
//! equal everywhere. The server enforces the dependency rule: an op is accepted only if every
//! dependency was applied.
//!
//! Seeds per run: 600 by default (the per-run floor of the epic); set `MELDFRAME_SEEDS` for more (the
//! nightly run). `MELDFRAME_SEED=n` replays one seed.

mod sim;

use sim::{Stats, run_seed};

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn seeds() -> std::ops::Range<u64> {
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(600),
    }
}

#[test]
fn replicas_converge_to_the_server() {
    let mut st = Stats::default();
    let range = seeds();
    let full = range.end - range.start >= 600;
    for seed in range {
        let n = 3 + (seed % 3) as usize;
        run_seed(seed, n, 300, seed % 2 == 0, &mut st);
    }
    println!("replicas_converge_to_the_server: {st:?}");
    if full {
        // The run must not be idle.
        assert!(st.rejects > 0 && st.dep_rejects > 0, "{st:?}");
        assert!(st.dropped_on_rebase > 0 && st.resent > 0, "{st:?}");
        // A server event delivered again must reach the replica and be ignored.
        assert!(st.duplicated > 0 && st.duplicates_ignored > 0, "{st:?}");
    }
}
