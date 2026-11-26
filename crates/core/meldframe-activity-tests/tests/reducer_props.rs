//! Property test: the rows, the lanes and the count are the same whatever the order in which a header and its ops
//! arrive. Ops arrive in the order of the log (a durable stream), headers (several revs of one `n` among them) at any
//! point and in any order; a batch with no header at all is part of the mix.
//!
//! Seeds per run: 600 by default, more with `MELDFRAME_SEEDS`; `MELDFRAME_SEED=n` replays one seed. Under WASM
//! the environment does not reach the test and the default is 20.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{Activity, RawHeader, Review};
use meldframe_revert::LoggedOp;

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

fn seeds() -> std::ops::Range<u64> {
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => {
            0..env("MELDFRAME_SEEDS").unwrap_or(if cfg!(target_arch = "wasm32") {
                20
            } else {
                600
            })
        }
    }
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

enum Event {
    Op(Box<LoggedOp>, u64),
    Header(RawHeader, u32),
}

/// The ops in log order with their arrival times, and the header frames (rev order, `n` shuffled).
type Scenario = (Vec<(LoggedOp, u64)>, Vec<(RawHeader, u32)>);

fn scenario(seed: u64) -> Scenario {
    let mut r = Rng(seed.wrapping_mul(0x9e37_79b9).wrapping_add(7));
    let actors = ["agent:a", "agent:b", "agent:c", ME];
    let (mut ops, mut headers, mut seq, mut t) = (Vec::new(), Vec::new(), 0u64, 1_000u64);
    let batches = 3 + r.below(6);
    let mut ns: Vec<u32> = (1..=batches as u32).collect();
    for i in (1..ns.len()).rev() {
        ns.swap(i, r.below(i + 1));
    }
    let mut rev = 0;
    for (i, n) in ns.into_iter().enumerate() {
        let actor = actors[r.below(3)];
        let prefix = format!("p{i}");
        let (len, first) = (1 + r.below(4) as u32, seq + 1);
        t += if r.below(4) == 0 { 400_000 } else { 1_000 };
        let at = t;
        for j in 0..len {
            seq += 1;
            t += 10;
            ops.push((
                op(
                    seq,
                    actor,
                    &format!("{prefix}:{j}"),
                    set_x("a", f64::from(j)),
                ),
                t,
            ));
        }
        if r.below(5) > 0 {
            for _ in 0..1 + r.below(3) {
                rev += 1;
                let mut h = header(n, rev, actor, &prefix, len + r.below(2) as u32, at);
                (h.first_seq, h.last_seq, h.written) = (first, seq, len.min(h.commands));
                if r.below(2) == 0 {
                    h.review = [Review::Accepted, Review::NeedsReview][r.below(2)];
                    (h.reviewed_by, h.reviewed_at_ms) = (Some(ME.into()), Some(at + 5));
                }
                headers.push((h, 1_000 - rev as u32));
            }
        }
        // The person's own edit between two batches.
        seq += 1;
        t += 500;
        ops.push((op(seq, ME, &format!("ed-{seq}"), set_x("a", 1.0)), t));
    }
    (ops, headers)
}

fn run(events: Vec<Event>, now: u64) -> (String, Option<u32>, u64) {
    let mut a: Activity = activity();
    for e in events {
        match e {
            Event::Op(o, t) => a.observe_op(&o, t),
            Event::Header(h, c) => {
                a.observe_header(h, c);
            }
        }
    }
    a.tick(now);
    (
        format!("{:?}\n{:?}", a.rows(), a.lanes()),
        a.unreviewed_count(),
        a.highest_rev(),
    )
}

#[test]
fn rows_lanes_and_count_do_not_depend_on_the_order_of_a_header_and_its_ops() {
    for seed in seeds() {
        let (ops, headers) = scenario(seed);
        let now = ops.last().map_or(0, |(_, t)| *t) + 1;
        let canonical: Vec<Event> = ops
            .iter()
            .map(|(o, t)| Event::Op(Box::new(o.clone()), *t))
            .chain(headers.iter().map(|(h, c)| Event::Header(h.clone(), *c)))
            .collect();
        let want = run(canonical, now);

        let mut r = Rng(seed ^ 0xabcd);
        let mut shuffled: Vec<Event> = ops
            .iter()
            .map(|(o, t)| Event::Op(Box::new(o.clone()), *t))
            .collect();
        for (h, c) in &headers {
            let at = r.below(shuffled.len() + 1);
            shuffled.insert(at, Event::Header(h.clone(), *c));
        }
        // Headers may also come before every op.
        let got = run(shuffled, now);
        assert_eq!(got, want, "seed {seed}");
    }
}
