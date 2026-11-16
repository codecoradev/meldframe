//! Property tests for the replica: random runs of several replicas against an in-memory server, with
//! the dependency rule, delayed delivery (random order across replicas), repeated delivery of ops to the
//! server and of events to the replicas, and refused ops. A replica also loses its connection now and
//! then: the events and refusals on the way to it are lost, and it loads the file again (`Replica::resync`)
//! and replays its pending ops. Events name the key of the op, as the wire does. Not modelled: a
//! reordered event.
//!
//! Checked on every seed:
//! - I1 convergence: when the queues are empty every replica equals the server, nothing is pending, the
//!   view is `base + pending` after every event, and sibling order is valid and equal everywhere.
//! - I3 and I5: undo, and redo, of a step that nobody else touched give the tree back exactly.
//! - I4: an undo or redo never overwrites a field another actor wrote and the author had seen.
//! - I7: an undo or redo the server refuses leaves history and view as they were; an undo and redo on
//!   a quiet replica are accepted.
//! - I8: a value an undo or redo writes was once the field's value (an inverse worked out from a
//!   refused premise would write a value that never was).
//! - The server enforces the dependency rule: an op is accepted only if every dependency was applied.
//!
//! Seeds per run: 600 by default (the per-run floor of the epic); set `MELDFRAME_SEEDS` for more (the
//! nightly run). `MELDFRAME_SEED=n` replays one seed. Under WASM the default is 20 seeds (`just wasm-test`;
//! the environment does not reach a WASM test, so the count cannot be raised there). The mutation check is in `tools/mutation-check-sync.sh`.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod sim;

use sim::{Stats, run_seed};

/// 600 natively (the per-run floor of the epic); few under WASM, which only checks that the same
/// arithmetic behaves the same and runs several times slower.
const DEFAULT_SEEDS: u64 = if cfg!(target_arch = "wasm32") {
    20
} else {
    600
};

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn seeds() -> std::ops::Range<u64> {
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(DEFAULT_SEEDS),
    }
}

#[test]
fn replicas_converge_and_undo_stays_per_actor() {
    let mut st = Stats::default();
    let range = seeds();
    let full = range.end - range.start >= 600;
    for seed in range {
        let n = 3 + (seed % 3) as usize;
        run_seed(seed, n, 300, seed % 2 == 0, &mut st);
    }
    println!("replicas_converge_and_undo_stays_per_actor: {st:?}");
    if full {
        // The run must not be idle.
        assert!(st.rejects > 0 && st.dep_rejects > 0, "{st:?}");
        assert!(st.dropped_on_rebase > 0 && st.resent > 0, "{st:?}");
        // A replica that lost its connection resynced from a snapshot and still converged.
        assert!(st.resyncs > 100, "{st:?}");
        // A server event delivered again must reach the replica and be ignored.
        assert!(st.duplicated > 0 && st.duplicates_ignored > 0, "{st:?}");
        // The undo and redo checks must not be idle either.
        assert!(
            st.undos > 1000 && st.redos > 300,
            "too little undo and redo: {st:?}"
        );
        assert!(st.exact_checks > 200, "{st:?}");
        assert!(st.i4_checked > 200, "{st:?}");
        assert!(st.i8_checked > 500, "{st:?}");
        assert!(st.sim_reject_checks > 500, "{st:?}");
        // A quiet replica that undoes and redoes with the server answering gets its tree back; the
        // counters are asserted, so the drill cannot pass by skipping every case.
        assert!(
            st.quiet_pairs > 1000,
            "too few quiet undo/redo pairs: {st:?}"
        );
        assert_eq!(
            st.quiet_pair_diffs, 0,
            "undo then redo changed the tree: {st:?}"
        );
    }
}

#[test]
fn a_single_writer_gets_every_step_back_exactly() {
    let mut st = Stats::default();
    for seed in 1000..1200u64 {
        run_seed(seed, 1, 400, false, &mut st);
    }
    println!("a_single_writer_gets_every_step_back_exactly: {st:?}");
    assert!(st.exact_checks > 500, "{st:?}");
}
