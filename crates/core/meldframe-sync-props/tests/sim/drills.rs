//! Drills at the end of a run: a refused step and an undo-redo pair on a quiet replica.

use super::{Net, Stats, snap};

/// I7 (simulated): on a quiet replica an undo or redo that the server refuses leaves the history and
/// the view exactly as they were.
pub(super) fn drill_refused_step(net: &mut Net, stats: &mut Stats) {
    for i in 0..net.reps.len() {
        for undoing in [true, false] {
            let r = &mut net.reps[i];
            if !(if undoing { r.can_undo() } else { r.can_redo() }) {
                continue;
            }
            let (d0, s0) = ((r.undo_depth(), r.redo_depth()), snap(r.nodes()));
            let did = if undoing {
                r.undo().unwrap()
            } else {
                r.redo().unwrap()
            };
            if !did {
                continue;
            }
            let ops = r.take_outbox();
            let d1 = (r.undo_depth(), r.redo_depth());
            let moved_one = if undoing {
                d1 == (d0.0 - 1, d0.1 + 1)
            } else {
                d1 == (d0.0 + 1, d0.1 - 1)
            };
            assert!(
                r.on_reject(&ops[0].key),
                "seed {}: the first op of a step is pending",
                net.seed
            );
            assert_eq!(
                r.pending_len(),
                0,
                "seed {}: a refused step falls whole",
                net.seed
            );
            r.check_consistency().unwrap();
            if moved_one {
                stats.sim_reject_checks += 1;
                let d2 = (r.undo_depth(), r.redo_depth());
                assert!(
                    d2 == d0 && snap(r.nodes()) == s0,
                    "seed {}: replica {i}: a refused {} changed history {d2:?} vs {d0:?} or the view",
                    net.seed,
                    if undoing { "undo" } else { "redo" }
                );
            }
        }
    }
}

/// Undo then redo on a quiet replica with the server answering: both are accepted and the pair
/// gives the tree back (no one else writes).
pub(super) fn drill_quiet_pair(net: &mut Net) {
    for i in 0..net.reps.len() {
        for _ in 0..3 {
            if !net.reps[i].can_undo() {
                break;
            }
            let s0 = snap(net.reps[i].nodes());
            let rejects = net.server.counts.rejects;
            if !net.reps[i].undo().unwrap() {
                break;
            }
            net.flush_all(i);
            net.quiesce();
            if net.server.counts.rejects != rejects || !net.reps[i].can_redo() {
                continue;
            }
            let rejects = net.server.counts.rejects;
            assert!(
                net.reps[i].redo().unwrap(),
                "seed {}: can_redo but redo failed",
                net.seed
            );
            net.flush_all(i);
            net.quiesce();
            if net.server.counts.rejects == rejects {
                net.stats.quiet_pairs += 1;
                net.stats.quiet_pair_diffs += u64::from(snap(net.reps[i].nodes()) != s0);
            }
        }
    }
}
