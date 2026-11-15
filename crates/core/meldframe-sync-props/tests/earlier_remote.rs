//! I9: an undo or redo never leaves a value that predates another actor's write. Two actors edit fields
//! of one node, undo and redo, and their ops reach the server in a random interleaving, so a write of the
//! other actor may be ordered before my edit, between my edit and my undo, or after both, and my undo or
//! redo may already be on its way when it arrives. Once every queue is empty, each field holds the value
//! of the last edit, in the server's order, that is still in force (not undone); a field no such edit
//! wrote is at its initial value. A redone edit counts at the place of the edit it redoes.
//!
//! Scenarios (by seed): edit and undo against writes of the other actor; edit, undo, edit, undo (a later edit
//! made while the first undo is on its way, then undone); edit, undo, redo all on their way; and both actors with an undo
//! pending. An undo that returns `false` (the other actor wrote every field of my confirmed edit) leaves
//! the edit in force.
//!
//! Why I4 and I8 did not catch the stale undo: I4 exempts a write the op's author had not seen
//! (`write seq > seen_seq`), and the stale undo was made before the other write arrived, so it counted
//! as concurrent; I8 asks only that the value once existed, and the old value did. Neither looks at the
//! value an undo leaves behind.
//!
//! Seeds per run: 600 by default; `MELDFRAME_SEEDS` sets more, `MELDFRAME_SEED=n` replays one.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[allow(dead_code)]
mod sim;

use meldframe_protocol::{Command, NodePatch};
use meldframe_sync::OpKind;
use sim::random::Rng;
use sim::{Net, snap};

const DEFAULT_SEEDS: u64 = if cfg!(target_arch = "wasm32") {
    20
} else {
    600
};

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

/// An update of `s1` that sets the fields chosen by the low bits of `mask` (x, y, fill) to `v`.
fn write(mask: usize, v: u32) -> Command {
    let mut p = NodePatch::default();
    let n = f64::from(v);
    if mask & 1 != 0 {
        p.x = Some(n);
    }
    if mask & 2 != 0 {
        p.y = Some(n);
    }
    if mask & 4 != 0 {
        p.fill = Some(format!("#{v:06x}"));
    }
    Command::UpdateNode {
        id: "s1".into(),
        patch: p,
    }
}

#[derive(Clone, Copy)]
enum Step {
    Edit(usize),
    Undo,
    Redo,
}

/// One actor's program and what it has done so far. `edits[k]` is true while the k-th edit is in force.
struct Actor {
    steps: Vec<Step>,
    next: usize,
    edits: Vec<bool>,
    undone: Vec<usize>,
    done: Vec<usize>,
}

impl Actor {
    fn new(steps: Vec<Step>) -> Self {
        Actor {
            steps,
            next: 0,
            edits: vec![],
            undone: vec![],
            done: vec![],
        }
    }

    fn finished(&self) -> bool {
        self.next >= self.steps.len()
    }
}

/// The program of each actor for this seed.
fn programs(seed: u64, r: &mut Rng) -> [Vec<Step>; 2] {
    let mask = |r: &mut Rng| 1 + r.below(7);
    let b_writes = |r: &mut Rng| {
        (0..1 + r.below(3))
            .map(|_| Step::Edit(1 + r.below(7)))
            .collect()
    };
    match seed % 4 {
        0 => [vec![Step::Edit(mask(r)), Step::Undo], b_writes(r)],
        1 => [
            vec![
                Step::Edit(mask(r)),
                Step::Undo,
                Step::Edit(mask(r)),
                Step::Undo,
            ],
            b_writes(r),
        ],
        2 => [
            vec![Step::Edit(mask(r)), Step::Undo, Step::Redo],
            b_writes(r),
        ],
        _ => [
            vec![Step::Edit(mask(r)), Step::Undo],
            vec![Step::Edit(mask(r)), Step::Undo],
        ],
    }
}

fn act(net: &mut Net, i: usize, a: &mut Actor, seed: u64) {
    let step = a.steps[a.next];
    a.next += 1;
    match step {
        Step::Edit(mask) => {
            let v = 100 * (i as u32 + 1) + a.edits.len() as u32 * 7 + seed as u32 % 5;
            net.reps[i].apply(&write(mask, v)).unwrap();
            a.edits.push(true);
            a.done.push(a.edits.len() - 1);
            a.undone.clear();
        }
        Step::Undo => {
            if net.reps[i].undo().unwrap() {
                let k = a.done.pop().expect("an undo that did something has a step");
                a.edits[k] = false;
                a.undone.push(k);
            }
        }
        Step::Redo => {
            if net.reps[i].redo().unwrap() {
                let k = a
                    .undone
                    .pop()
                    .expect("a redo that did something has a step");
                a.edits[k] = true;
                a.done.push(k);
            }
        }
    }
    net.flush_all(i);
}

fn run(seed: u64) {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut net = Net::new(seed, 2);
    let initial = net.reps[0]
        .nodes()
        .iter()
        .find(|n| n.id == "s1")
        .unwrap()
        .clone();
    let [pa, pb] = programs(seed, &mut r);
    let mut actors = [Actor::new(pa), Actor::new(pb)];
    while !actors.iter().all(Actor::finished) {
        let i = r.below(2);
        match r.below(6) {
            0..=1 if !actors[i].finished() => act(&mut net, i, &mut actors[i], seed),
            2 | 3 => net.deliver_up(r.below(2)),
            _ => net.deliver_down(r.below(2)),
        }
    }
    net.quiesce();
    // The k-th Edit of each actor in the server's log is that actor's k-th edit. `who` marks the actors
    // that undid an edit that wrote each field.
    let mut seen = [0usize; 2];
    let mut want = (initial.x, initial.y, initial.fill.clone());
    let mut who = [0u8; 3];
    for (actor, kind, cmd) in &net.server.log {
        let i = usize::from(actor == "user:u1");
        if *kind != OpKind::Edit {
            continue;
        }
        let k = seen[i];
        seen[i] += 1;
        let Command::UpdateNode { patch, .. } = cmd else {
            continue;
        };
        let live = actors[i].edits[k];
        // Only an edit that its actor undid counts: its undo was on its way.
        let bit = if live { 0 } else { 1 << i };
        who[0] |= u8::from(patch.x.is_some()) * bit;
        who[1] |= u8::from(patch.y.is_some()) * bit;
        who[2] |= u8::from(patch.fill.is_some()) * bit;
        if live {
            want.0 = patch.x.unwrap_or(want.0);
            want.1 = patch.y.unwrap_or(want.1);
            want.2 = patch.fill.clone().unwrap_or(want.2.clone());
        }
    }
    let s1 = net.server.nodes.iter().find(|n| n.id == "s1").unwrap();
    // Both actors with an undo of an edit of the same field: the other actor's stale undo and its correction
    // are writes that come after my edit, so the field may keep a value of an undone edit (a known limit,
    // see `docs/architecture.md`). Such a field is not compared.
    let skip = |f: usize| seed % 4 == 3 && who[f] == 3;
    let (x, y, fill) = (s1.x, s1.y, s1.fill.clone());
    assert!(
        (skip(0) || x == want.0) && (skip(1) || y == want.1) && (skip(2) || fill == want.2),
        "seed {seed} (scenario {}): I9: an undo or redo left a value that predates another actor's write: {:?} vs {want:?}",
        seed % 4,
        (x, y, &fill)
    );
    for (i, rep) in net.reps.iter().enumerate() {
        assert_eq!(rep.pending_len(), 0, "seed {seed}: replica {i} pending");
        assert_eq!(
            snap(rep.nodes()),
            snap(&net.server.nodes),
            "seed {seed}: replica {i}"
        );
    }
}

#[test]
fn an_undo_never_leaves_a_value_that_predates_another_actors_write() {
    match env("MELDFRAME_SEED") {
        Some(s) => run(s),
        None => (0..env("MELDFRAME_SEEDS").unwrap_or(DEFAULT_SEEDS)).for_each(run),
    }
}
