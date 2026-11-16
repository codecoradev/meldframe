//! Replicas fed by the real room converge. The same random runs as `props.rs` (random edits, undo and
//! redo, delayed delivery and ops sent to the room twice) but the server is `meldframe-host`: every op goes through the
//! gate, and the room accepts it with its permit. Checked on every seed: when the queues are empty every
//! replica equals the room, nothing is pending, and the log of the room replays to the same tree; and the
//! room never writes an op whose dependencies were not written for the same actor first.
//!
//! Even seeds run a gate that allows deletes; odd seeds run the default policy, which refuses a delete
//! with `forbidden`, so the replicas also see refusals that come from the gate. Seeds per run: 600 by
//! default; `MELDFRAME_SEEDS` sets more, `MELDFRAME_SEED=n` replays one. Under WASM the default is 20 seeds
//! (`just wasm-test`; the environment does not reach a WASM test).

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "sim/random.rs"]
mod random;

use meldframe_host::{Gate, MemoryStore, Op, Principal, Room, Store};
use meldframe_module::{Policy, default_policy};
use meldframe_protocol::{Command, Node};
use meldframe_sync::{Outgoing, Replica, ReplicaError};
use random::{Rng, initial, rand_edit};
use std::collections::{BTreeMap, HashSet, VecDeque};

const DEFAULT_SEEDS: u64 = if cfg!(target_arch = "wasm32") {
    20
} else {
    600
};
const WS: &str = "ws_1";
const FILE: &str = "file_1";

// A message in flight: the size difference is irrelevant in a test.
#[allow(clippy::large_enum_variant)]
enum Msg {
    Event {
        seq: u64,
        actor: String,
        key: String,
        cmd: Command,
    },
    Reject {
        key: String,
    },
}

#[derive(Default, Debug)]
struct Stats {
    accepted: u64,
    duplicates: u64,
    resyncs: u64,
    /// Of those, the ones that followed `ReplicaError::Diverged`.
    diverged: u64,
    refused: BTreeMap<&'static str, u64>,
}

struct Net {
    gate: Gate,
    room: Room<MemoryStore>,
    reps: Vec<Replica>,
    names: Vec<String>,
    up: Vec<VecDeque<Outgoing>>,
    down: Vec<VecDeque<Msg>>,
    next_id: Vec<u32>,
    /// `(actor, key)` of every op the room wrote, to check the dependency rule from outside.
    applied: HashSet<(String, String)>,
    seed: u64,
    /// Even seeds run a gate that allows deletes, so `forbidden` is not expected there.
    allow: bool,
    stats: Stats,
}

fn snap(n: &[Node]) -> String {
    let mut v = n.to_vec();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&v).unwrap()
}

fn submit(
    gate: &mut Gate,
    room: &mut Room<MemoryStore>,
    actor: &str,
    op: Op,
) -> Result<(u64, bool), &'static str> {
    let who = Principal::new(WS, actor);
    let a = gate
        .submit(&who, vec![op])
        .map_err(|r| r.code())?
        .admitted
        .remove(0);
    let done = room.accept(a).map_err(|e| match e {
        meldframe_host::RoomError::Refused(r) => r.code(),
        other => panic!("the room failed: {other}"),
    })?;
    Ok((done.seq, done.written))
}

impl Net {
    fn new(seed: u64, n: usize) -> Net {
        let allow = seed.is_multiple_of(2);
        let mut gate = Gate::with_policy(move |_, tier| {
            if allow {
                Policy::Auto
            } else {
                default_policy(tier)
            }
        });
        let mut room = Room::open(WS, FILE, MemoryStore::new()).unwrap();
        for (i, node) in initial().into_iter().enumerate() {
            let cmd = Command::AddNode { node, index: None };
            submit(
                &mut gate,
                &mut room,
                "system",
                Op::new(format!("seed{i}"), cmd),
            )
            .unwrap();
        }
        // On odd seeds the last replica is a second session of replica 0's actor: its ops and its events
        // look alike, and only the idempotency key tells them apart.
        let twin = seed % 2 == 1 && n >= 3;
        let names: Vec<String> = (0..n)
            .map(|i| format!("user:u{}", if twin && i == n - 1 { 0 } else { i }))
            .collect();
        let reps = names
            .iter()
            .enumerate()
            .map(|(i, a)| {
                Replica::new(a, &format!("c{i}"), room.nodes().to_vec(), room.seq()).unwrap()
            })
            .collect();
        Net {
            gate,
            room,
            reps,
            names,
            up: (0..n).map(|_| VecDeque::new()).collect(),
            down: (0..n).map(|_| VecDeque::new()).collect(),
            next_id: vec![0; n],
            applied: HashSet::new(),
            seed,
            allow,
            stats: Stats::default(),
        }
    }

    fn flush(&mut self, i: usize) {
        let out = self.reps[i].take_outbox();
        self.up[i].extend(out);
    }

    fn deliver_up(&mut self, i: usize, r: &mut Rng) {
        let Some(o) = self.up[i].pop_front() else {
            return;
        };
        let op = Op::new(o.key.clone(), o.command.clone()).with_deps(o.deps.clone());
        match submit(&mut self.gate, &mut self.room, &self.names[i], op) {
            Ok((_, false)) => self.stats.duplicates += 1,
            Ok((seq, true)) => {
                self.stats.accepted += 1;
                // The dependency rule: every dependency of a written op was written for this actor before.
                for d in &o.deps {
                    let key = (self.names[i].clone(), d.clone());
                    assert!(
                        self.applied.contains(&key),
                        "seed {}: the room accepted {} with the unapplied dependency {d}",
                        self.seed,
                        o.key
                    );
                }
                self.applied.insert((self.names[i].clone(), o.key.clone()));
                for d in self.down.iter_mut() {
                    d.push_back(Msg::Event {
                        seq,
                        actor: self.names[i].clone(),
                        key: o.key.clone(),
                        cmd: o.command.clone(),
                    });
                }
            }
            Err(code) => {
                // Only these refusals are expected: a delete under the default policy, a dependency
                // that was never written, and a command the document core refuses. Anything else
                // (a store error, a reused key, a conflict, a quota) is a regression in the room.
                let expected = code == "dependency_rejected"
                    || code.starts_with("E_")
                    || (code == "forbidden" && !self.allow);
                assert!(expected, "seed {}: unexpected refusal {code}", self.seed);
                *self.stats.refused.entry(code).or_default() += 1;
                self.down[i].push_back(Msg::Reject { key: o.key.clone() });
            }
        }
        // About one op in six reaches the room again (a resend).
        if r.below(6) == 0 {
            self.up[i].push_front(o);
        }
    }

    fn deliver_down(&mut self, i: usize) {
        let Some(m) = self.down[i].pop_front() else {
            return;
        };
        match m {
            Msg::Reject { key } => {
                self.reps[i].on_reject(&key);
            }
            Msg::Event {
                seq,
                actor,
                key,
                cmd,
            } => {
                match self.reps[i].on_event_keyed(seq, &actor, Some(&key), &cmd) {
                    Ok(_) => {}
                    // An op of mine was ordered ahead of an older one: the platform loads the file again.
                    Err(ReplicaError::Diverged(_)) => {
                        self.stats.diverged += 1;
                        return self.resync(i);
                    }
                    Err(e) => panic!("seed {}: replica {i}: on_event: {e:?}", self.seed),
                }
            }
        }
        if let Err(e) = self.reps[i].check_consistency() {
            panic!("seed {}: replica {i}: {e}", self.seed);
        }
        self.flush(i);
    }

    /// Replica `i` lost its connection and the messages in flight to it: it loads the file again and
    /// replays the ops the room does not hold yet.
    fn resync(&mut self, i: usize) {
        let applied: Vec<String> = self
            .applied
            .iter()
            .filter(|(a, _)| *a == self.names[i])
            .map(|(_, k)| k.clone())
            .collect();
        self.down[i].clear();
        let (nodes, seq) = (self.room.nodes().to_vec(), self.room.seq());
        if let Err(e) = self.reps[i].resync(nodes, seq, &applied) {
            panic!("seed {}: replica {i}: resync: {e:?}", self.seed);
        }
        if let Err(e) = self.reps[i].check_consistency() {
            panic!("seed {}: replica {i}: {e}", self.seed);
        }
        self.stats.resyncs += 1;
        self.flush(i);
    }

    fn act(&mut self, i: usize, r: &mut Rng) {
        match r.below(20) {
            0..=13 => {
                let nodes = self.reps[i].nodes().to_vec();
                let c = rand_edit(&nodes, i, &mut self.next_id[i], r);
                let _ = self.reps[i].apply(&c);
            }
            14..=17 => {
                let _ = self.reps[i].undo().unwrap();
            }
            _ => {
                let _ = self.reps[i].redo().unwrap();
            }
        }
        self.flush(i);
    }

    fn quiesce(&mut self, r: &mut Rng) {
        while (0..self.reps.len()).any(|i| !self.up[i].is_empty() || !self.down[i].is_empty()) {
            for i in 0..self.reps.len() {
                // A resend is put back at the front with probability 1/6, so this ends with probability 1.
                while !self.up[i].is_empty() {
                    self.deliver_up(i, r);
                }
            }
            for i in 0..self.reps.len() {
                while !self.down[i].is_empty() {
                    self.deliver_down(i);
                }
            }
        }
    }

    fn check(&self) {
        let want = snap(self.room.nodes());
        for (i, rep) in self.reps.iter().enumerate() {
            assert_eq!(
                rep.pending_len(),
                0,
                "seed {}: replica {i} has pending ops",
                self.seed
            );
            assert_eq!(
                snap(rep.nodes()),
                want,
                "seed {}: replica {i} differs from the room",
                self.seed
            );
            assert_eq!(
                rep.seq(),
                self.room.seq(),
                "seed {}: replica {i} lags on the room's sequence number",
                self.seed
            );
            assert_eq!(
                snap(rep.base()),
                want,
                "seed {}: replica {i} base differs",
                self.seed
            );
        }
        // The log replays to the same tree.
        let log = self.room.store().ops_after(FILE, 0, usize::MAX).unwrap();
        let mut nodes = Vec::new();
        for op in &log {
            meldframe_core::apply_command(
                &mut nodes,
                &op.command,
                &meldframe_core::Limits::default(),
            )
            .unwrap_or_else(|e| {
                panic!("seed {}: log op {} does not replay: {e}", self.seed, op.seq)
            });
        }
        assert_eq!(
            snap(&nodes),
            want,
            "seed {}: the log does not replay to the room",
            self.seed
        );
    }
}

fn run_seed(seed: u64, st: &mut Stats) {
    let n = 2 + (seed % 3) as usize;
    let mut net = Net::new(seed, n);
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for _ in 0..300 {
        let i = r.below(n);
        match r.below(8) {
            0..=3 => net.act(i, &mut r),
            4..=5 => net.deliver_up(i, &mut r),
            6 if r.below(12) == 0 => net.resync(i),
            _ => net.deliver_down(i),
        }
    }
    net.quiesce(&mut r);
    net.check();
    st.accepted += net.stats.accepted;
    st.duplicates += net.stats.duplicates;
    st.resyncs += net.stats.resyncs;
    st.diverged += net.stats.diverged;
    for (k, v) in net.stats.refused {
        *st.refused.entry(k).or_default() += v;
    }
}

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

#[test]
fn replicas_fed_by_the_room_converge() {
    let range = match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(DEFAULT_SEEDS),
    };
    let full = range.end - range.start >= 600;
    // Under WASM the run is short and the activity floors below do not apply.
    let mut st = Stats::default();
    for seed in range {
        run_seed(seed, &mut st);
    }
    println!("replicas_fed_by_the_room_converge: {st:?}");
    if full {
        // The run must not be idle: ops were accepted, resent ones were recognised, and the gate and the
        // room refused for the reasons the run is meant to exercise.
        assert!(st.accepted > 5000, "{st:?}");
        assert!(st.duplicates > 500, "{st:?}");
        assert!(st.resyncs > 100, "{st:?}");
        for code in ["forbidden", "dependency_rejected"] {
            assert!(
                st.refused.get(code).copied().unwrap_or(0) > 0,
                "no {code}: {st:?}"
            );
        }
    }
}
