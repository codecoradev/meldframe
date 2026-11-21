//! Property test for the variable commands: random runs of two or three replicas and the real gate and room of
//! `meldframe-host`. Until now no random run made a variable command, so a rename was covered by scenarios only.
//!
//! A step is one of: a random edit of a replica (rename of a variable, a mode or a collection, add or remove a
//! variable, a mode or a collection, set a value, an alias among them), undo, redo, send the outbox to the room,
//! deliver some of what the room accepted or refused. Names come from small alphabets, so two variables of one
//! collection often want the same name and the room refuses the second; an id that does not exist and an
//! invalid name come now and then. Odd seeds are mostly renames of variables. About one step in seventeen is a
//! race: one replica renames something, another takes it in and renames or removes it, and the first undoes (and
//! may redo), which a random order alone meets too rarely.
//!
//! Checked after every step:
//! - every replica is consistent (the view is the confirmed state plus the pending ops) and holds variables that
//!   pass `check_variables`, as the room does;
//! - a refused edit, undo or redo leaves the replica as it was, and a refused op leaves the room as it was;
//! - an undo or a redo never queues a rename of a name that another actor wrote after the author's step and that
//!   the author has seen (last writer wins on the name);
//! - a replica with nothing pending that has every event equals the room.
//!
//! At the end the queues are drained and every replica equals the room. A second test runs one writer and checks
//! that undo and redo give the variables back exactly.
//!
//! Seeds per run: 200 by default; set `MELDFRAME_SEEDS` for more. `MELDFRAME_SEED=n` replays one seed. Under WASM the
//! default is 20 seeds (`just wasm-test`; the environment does not reach a WASM test, so the count cannot be raised
//! there). A failure prints the seed, the last 30 steps (`MELDFRAME_TRACE=n` for n) and the line that replays it.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod sim;

use meldframe_protocol::Variables;
use sim::random::Rng;
use sim::{Sim, Stats};

const DEFAULT_SEEDS: u64 = if cfg!(target_arch = "wasm32") {
    20
} else {
    200
};
const STEPS: usize = 200;

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn seeds() -> std::ops::Range<u64> {
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(DEFAULT_SEEDS),
    }
}

fn run_seed(seed: u64, st: &mut Stats) {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let n = 2 + (seed / 2 % 2) as usize;
    let renames = seed % 2 == 1;
    let mut sim = Sim::new(seed, n);
    for _ in 0..STEPS {
        let i = r.below(n);
        match r.below(100) {
            0..=5 => sim.race(i, (i + 1 + r.below(n - 1)) % n, &mut r),
            6..=57 => drop(sim.edit(i, &mut r, renames)),
            58..=67 => drop(sim.undo(i)),
            68..=75 => drop(sim.redo(i)),
            76..=87 => sim.send(i),
            _ => {
                let k = 1 + r.below(4);
                sim.deliver(i, k);
            }
        }
        sim.check();
    }
    sim.settle();
    let s = sim.stats;
    st.seeds += 1;
    st.renames_accepted += s.renames_accepted;
    st.duplicate_name_refusals += s.duplicate_name_refusals;
    st.rename_refusals += s.rename_refusals;
    st.local_refusals += s.local_refusals;
    st.room_refusals += s.room_refusals;
    st.undos += s.undos;
    st.redos += s.redos;
    st.undo_renames += s.undo_renames;
    st.dropped += s.dropped;
}

#[test]
fn replicas_and_room_agree_under_random_variable_edits() {
    let mut st = Stats::default();
    let range = seeds();
    let full = range.end - range.start >= 100;
    for seed in range {
        run_seed(seed, &mut st);
    }
    println!("replicas_and_room_agree_under_random_variable_edits: {st:?}");
    if full {
        // The run must not be idle: the room refuses names that are taken, renames go through, undo and
        // redo run, and ops fall from a queue when another actor's op comes in.
        let n = st.seeds;
        assert!(st.renames_accepted > 20 * n, "{st:?}");
        assert!(st.duplicate_name_refusals > n, "{st:?}");
        assert!(st.room_refusals > 5 * n, "{st:?}");
        assert!(st.local_refusals > 5 * n, "{st:?}");
        assert!(st.undos > 5 * n && st.redos > n / 2, "{st:?}");
        assert!(st.undo_renames > n, "{st:?}");
        assert!(st.dropped > n, "{st:?}");
    }
}

/// One writer: an undo gives back exactly the variables before the step, a redo exactly the ones after it.
/// (The runs above only compare replicas with each other; this one compares a replica with its own past.)
#[test]
fn a_single_writer_gets_every_step_back_exactly() {
    let mut checked = 0u64;
    for seed in seeds() {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let renames = seed % 2 == 1;
        let mut sim = Sim::new(seed, 1);
        // Variables before each step that can be undone, and after each step that can be redone.
        let (mut undo, mut redo): (Vec<Variables>, Vec<Variables>) = (vec![], vec![]);
        for _ in 0..STEPS {
            match r.below(100) {
                0..=59 => {
                    let (before, depth) = (sim.canon(0), sim.undo_depth(0));
                    // An edit that changes nothing, or is refused, makes no step.
                    if sim.edit(0, &mut r, renames) && sim.undo_depth(0) > depth {
                        undo.push(before);
                        redo.clear();
                    }
                }
                60..=79 => {
                    let now = sim.canon(0);
                    if sim.undo(0) {
                        sim.expect_vars(0, &undo.pop().expect("a step was recorded"), "undo");
                        redo.push(now);
                        checked += 1;
                    }
                }
                80..=89 => {
                    let now = sim.canon(0);
                    if sim.redo(0) {
                        sim.expect_vars(0, &redo.pop().expect("a step was undone"), "redo");
                        undo.push(now);
                        checked += 1;
                    }
                }
                90..=94 => sim.send(0),
                _ => sim.deliver(0, 1 + r.below(4)),
            }
            sim.check();
        }
        sim.settle();
    }
    println!("a_single_writer_gets_every_step_back_exactly: {checked} exact checks");
    let n = seeds();
    if n.end - n.start >= 100 {
        assert!(
            checked > 5 * (n.end - n.start),
            "too few exact checks: {checked}"
        );
    }
}
