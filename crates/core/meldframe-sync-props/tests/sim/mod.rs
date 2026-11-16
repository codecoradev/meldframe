//! Several replicas and one server, joined by queues. Each queue is in order, but which queue moves next is
//! random (delay), ops are sometimes sent to the server twice (resend) and events are sometimes delivered
//! to a replica twice. A lost or reordered event is not modelled: it needs the resync entry point. Every seed is one run; a failure names the seed, and `MELDFRAME_SEED=n` replays it.

mod drills;
pub mod random;
mod resync;
pub mod server;

use meldframe_core::normalize_positions;
use meldframe_order::fracindex;
use meldframe_protocol::{Command, Node};
use meldframe_sync::{EventOutcome, Outgoing, Replica, ReplicaError};
use random::{Rng, initial, rand_edit};
use server::{Server, Verdict};
use std::collections::{HashSet, VecDeque};

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Msg {
    Event {
        seq: u64,
        actor: String,
        key: String,
        cmd: Command,
    },
    Reject {
        key: String,
        /// The refusal travels apart from the stream: it may arrive behind later events.
        late: bool,
    },
}

/// What a run did, to show that the checks were not idle.
#[derive(Default, Debug)]
pub struct Stats {
    pub seeds: u64,
    pub undos: u64,
    pub redos: u64,
    pub dropped_on_rebase: u64,
    pub resent: u64,
    pub exact_checks: u64,
    pub sim_reject_checks: u64,
    pub quiet_pairs: u64,
    pub quiet_pair_diffs: u64,
    pub rejects: u64,
    pub dep_rejects: u64,
    /// Replicas that loaded a snapshot again and replayed their pending ops.
    pub resyncs: u64,
    /// Replicas that could not follow an event (`ReplicaError::Diverged`): zero on a healthy run.
    pub diverged: u64,
    pub late_rejects: u64,
    /// Pairs of edits that reached the server in the other order.
    pub reordered: u64,
    /// Events delivered to a replica a second time.
    pub duplicated: u64,
    /// Of those, the ones the replica ignored as a duplicate.
    pub duplicates_ignored: u64,
    pub ur_accepted: u64,
    pub i4_checked: u64,
    pub i4_concurrent: u64,
    pub i8_checked: u64,
}

/// The tree in a canonical form (list order has no meaning), `pos` included.
pub fn snap(n: &[Node]) -> String {
    let mut v = n.to_vec();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&v).unwrap()
}

/// `pos` is valid and unique among the children of one parent.
fn check_pos(n: &[Node], what: &str, seed: u64) {
    let mut seen: HashSet<(Option<&str>, &str)> = HashSet::new();
    for x in n {
        assert!(
            fracindex::is_valid(&x.pos),
            "seed {seed}: {what}: bad pos {:?} on {}",
            x.pos,
            x.id
        );
        assert!(
            seen.insert((x.parent_id.as_deref(), x.pos.as_str())),
            "seed {seed}: {what}: duplicate pos {:?} under {:?}",
            x.pos,
            x.parent_id
        );
    }
}

/// For I3 and I5: the tree before each step that nobody else touched, so an undo or redo of it must
/// give that tree back exactly.
#[derive(Default)]
struct Mirror {
    clean_undo: Vec<String>,
    clean_redo: Vec<String>,
}

impl Mirror {
    fn clear(&mut self) {
        self.clean_undo.clear();
        self.clean_redo.clear();
    }
}

pub struct Net {
    pub server: Server,
    pub reps: Vec<Replica>,
    names: Vec<String>,
    up: Vec<VecDeque<Outgoing>>,
    down: Vec<VecDeque<Msg>>,
    mirrors: Vec<Mirror>,
    pub stats: Stats,
    /// Ops of one replica may reach the server in the other order (the `earlier_remote` runs rely on the order).
    pub reorder: bool,
    /// Deliveries so far: a run that does not settle is a failure, not a hang.
    steps: u64,
    seed: u64,
    next_id: Vec<u32>,
    trace: Vec<Vec<String>>,
}

impl Net {
    pub fn new(seed: u64, n: usize) -> Net {
        let mut init = initial();
        normalize_positions(&mut init);
        let names: Vec<String> = (0..n).map(|i| format!("user:u{i}")).collect();
        Net {
            server: Server::new(init.clone(), seed),
            reps: names
                .iter()
                .enumerate()
                .map(|(i, a)| Replica::new(a, &format!("c{i}"), init.clone(), 0).unwrap())
                .collect(),
            names,
            up: (0..n).map(|_| VecDeque::new()).collect(),
            down: (0..n).map(|_| VecDeque::new()).collect(),
            mirrors: (0..n).map(|_| Mirror::default()).collect(),
            stats: Stats::default(),
            reorder: false,
            steps: 0,
            seed,
            next_id: vec![0; n],
            trace: vec![vec![]; n],
        }
    }

    pub(super) fn fail(&self, i: usize, msg: &str) -> ! {
        let t = &self.trace[i];
        let tail = t[t.len().saturating_sub(30)..].join("\n");
        panic!("seed {}: replica {i}: {msg}\n{tail}", self.seed)
    }

    fn flush(&mut self, i: usize) {
        for o in self.reps[i].take_outbox() {
            let c = serde_json::to_string(&o.command).unwrap();
            self.trace[i].push(format!(
                "OUT {:?} {}",
                o.kind,
                c.chars().take(120).collect::<String>()
            ));
            self.up[i].push_back(o);
        }
    }

    /// The server takes the oldest op of replica `i`.
    pub fn deliver_up(&mut self, i: usize) {
        self.steps += 1;
        if self.steps > 20_000 {
            self.fail(i, "the run does not settle");
        }
        // Two ops of one replica sometimes reach the server in the other order.
        if self.reorder
            && self.up[i].len() >= 2
            && (self.seed ^ self.server.seq ^ i as u64).is_multiple_of(9)
        {
            self.up[i].swap(0, 1);
            self.stats.reordered += 1;
        }
        let Some(o) = self.up[i].pop_front() else {
            return;
        };
        match self.server.submit(&self.names[i], &o) {
            Verdict::Duplicate => {}
            Verdict::Rejected => {
                let late = (self.seed ^ self.server.seq).is_multiple_of(3);
                self.down[i].push_back(Msg::Reject { key: o.key, late });
            }
            Verdict::Accepted { seq } => {
                for d in self.down.iter_mut() {
                    d.push_back(Msg::Event {
                        seq,
                        actor: self.names[i].clone(),
                        key: o.key.clone(),
                        cmd: o.command.clone(),
                    });
                }
            }
        }
    }

    /// Replica `i` takes the oldest message of the server.
    pub fn deliver_down(&mut self, i: usize) {
        self.steps += 1;
        if self.steps > 20_000 {
            self.fail(i, "the run does not settle");
        }
        let Some(m) = self.down[i].pop_front() else {
            return;
        };
        match m {
            Msg::Reject { key, late: true } if !self.down[i].is_empty() => {
                self.stats.late_rejects += 1;
                self.down[i].push_back(Msg::Reject { key, late: false });
                return;
            }
            Msg::Reject { key, .. } => {
                if self.reps[i].on_reject(&key) {
                    self.mirrors[i].clear();
                }
            }
            Msg::Event {
                seq,
                actor,
                key,
                cmd,
            } => {
                let res = match self.reps[i].on_event_keyed(seq, &actor, Some(&key), &cmd) {
                    Ok(res) => res,
                    // The replica cannot follow: the platform loads the file again.
                    Err(ReplicaError::Diverged(_)) => {
                        self.stats.diverged += 1;
                        return self.resync(i);
                    }
                    Err(e) => self.fail(i, &format!("on_event: {e:?}")),
                };
                self.trace[i].push(format!("EV seq{seq} {actor} -> {res:?}"));
                match res {
                    EventOutcome::Rebased { dropped } => {
                        self.stats.dropped_on_rebase += dropped as u64;
                        self.mirrors[i].clear();
                    }
                    EventOutcome::Duplicate => self.stats.duplicates_ignored += 1,
                    _ => {}
                }
                // About one event in seven reaches the replica again, later (the queue stays in order, so
                // the copy arrives behind newer events or right behind the original).
                if matches!(res, EventOutcome::Confirmed | EventOutcome::Rebased { .. })
                    && (self.seed
                        ^ seq.wrapping_mul(2_654_435_761)
                        ^ (i as u64).wrapping_mul(40_503))
                    .is_multiple_of(7)
                {
                    self.down[i].push_back(Msg::Event {
                        seq,
                        actor,
                        key,
                        cmd,
                    });
                    self.stats.duplicated += 1;
                }
            }
        }
        if let Err(e) = self.reps[i].check_consistency() {
            self.fail(i, &e);
        }
        self.flush(i);
    }

    /// Replica `i` edits (one command or one gesture), undoes or redoes.
    pub fn act(&mut self, i: usize, r: &mut Rng) {
        let before = snap(self.reps[i].nodes());
        match r.below(20) {
            0..=10 => {
                let grouped = r.below(4) == 0;
                if grouped {
                    self.reps[i].begin_group();
                }
                let mut ok = 0;
                for _ in 0..(if grouped { 2 + r.below(3) } else { 1 }) {
                    let nodes = self.reps[i].nodes().to_vec();
                    let c = rand_edit(&nodes, i, &mut self.next_id[i], r);
                    ok += usize::from(self.reps[i].apply(&c).is_ok());
                }
                if grouped {
                    self.reps[i].end_group();
                }
                if ok > 0 {
                    let m = &mut self.mirrors[i];
                    m.clean_undo.push(before);
                    m.clean_redo.clear();
                }
            }
            11..=15 => {
                let clean = self.mirrors[i].clean_undo.pop();
                let did = self.reps[i].undo().unwrap();
                self.trace[i].push(format!("UNDO did={did}"));
                if did {
                    self.stats.undos += 1;
                    match clean {
                        Some(s) => {
                            self.stats.exact_checks += 1;
                            if snap(self.reps[i].nodes()) != s {
                                self.fail(
                                    i,
                                    "I3: an undo of an untouched step did not restore the tree",
                                );
                            }
                            self.mirrors[i].clean_redo.push(before);
                        }
                        None => self.mirrors[i].clear(),
                    }
                } else if clean.is_some() {
                    self.fail(i, "undo failed although a clean step exists");
                }
            }
            _ => {
                let redo_before = self.reps[i].redo_depth();
                let did = self.reps[i].redo().unwrap();
                self.trace[i].push(format!("REDO did={did}"));
                if did {
                    self.stats.redos += 1;
                    let m = &mut self.mirrors[i];
                    if redo_before - self.reps[i].redo_depth() != 1 {
                        m.clear();
                    }
                    if let Some(s) = self.mirrors[i].clean_redo.pop() {
                        self.stats.exact_checks += 1;
                        if snap(self.reps[i].nodes()) != s {
                            self.fail(
                                i,
                                "I5: a redo of an untouched step did not give the tree back",
                            );
                        }
                    }
                    self.mirrors[i].clean_undo.clear();
                } else {
                    self.mirrors[i].clear();
                }
            }
        }
        self.flush(i);
    }

    /// Deliver everything until nothing moves.
    pub fn quiesce(&mut self) {
        loop {
            let mut moved = false;
            for i in 0..self.reps.len() {
                while !self.up[i].is_empty() {
                    self.deliver_up(i);
                    moved = true;
                }
            }
            for i in 0..self.reps.len() {
                while !self.down[i].is_empty() {
                    self.deliver_down(i);
                    moved = true;
                }
            }
            if !moved {
                return;
            }
        }
    }

    /// Resend every op that is not confirmed (same keys).
    pub fn resend(&mut self, i: usize) {
        for o in self.reps[i].unconfirmed() {
            self.up[i].push_back(o);
            self.stats.resent += 1;
        }
    }

    pub fn flush_all(&mut self, i: usize) {
        self.flush(i);
    }
}

/// I1: every replica equals the server, nothing is pending, sibling order is valid and equal.
fn check_converged(net: &Net, when: &str) {
    let seed = net.seed;
    let sj = snap(&net.server.nodes);
    check_pos(&net.server.nodes, "server", seed);
    for (i, rep) in net.reps.iter().enumerate() {
        check_pos(rep.nodes(), &format!("replica {i}"), seed);
        assert_eq!(
            rep.pending_len(),
            0,
            "seed {seed}: {when}: replica {i} still has pending ops"
        );
        assert_eq!(
            snap(rep.nodes()),
            sj,
            "seed {seed}: {when}: replica {i} did not converge"
        );
        assert_eq!(
            snap(rep.base()),
            sj,
            "seed {seed}: {when}: base of replica {i} differs"
        );
        assert_eq!(
            rep.seq(),
            net.server.seq,
            "seed {seed}: {when}: seq of replica {i}"
        );
    }
}

/// One run: random steps, then the checks.
pub fn run_seed(seed: u64, n: usize, steps: usize, quiet_windows: bool, stats: &mut Stats) {
    run_seed_with(seed, n, steps, quiet_windows, false, stats);
}

/// With `reorder`, two edits of one replica sometimes reach the server in the other order. A refusal of
/// an undo step then happens more often than the exact put-back drill allows (a step refused twice is
/// dropped on purpose), so that drill is left out.
pub fn run_seed_with(
    seed: u64,
    n: usize,
    steps: usize,
    quiet_windows: bool,
    reorder: bool,
    stats: &mut Stats,
) {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut net = Net::new(seed, n);
    net.reorder = reorder;
    for step in 0..steps {
        // A quiet window: only replica 0 writes and delivery is prompt (I3 on a clean path).
        let quiet = quiet_windows && (step / 40) % 2 == 1;
        match r.below(10) {
            0..=3 => {
                let i = if quiet { 0 } else { r.below(n) };
                net.act(i, &mut r);
            }
            4..=6 => net.deliver_up(r.below(n)),
            7..=8 => net.deliver_down(r.below(n)),
            _ if r.below(8) == 0 => net.resend(r.below(n)),
            _ if r.below(40) == 0 => net.resync(r.below(n)),
            _ => {}
        }
        if quiet && step % 40 == 39 {
            net.quiesce();
        }
    }
    net.quiesce();
    check_converged(&net, "after the run");
    if !reorder {
        drills::drill_refused_step(&mut net, stats);
    }
    drills::drill_quiet_pair(&mut net);
    check_converged(&net, "after the drills");
    let c = &net.server.counts;
    let s = &net.stats;
    stats.seeds += 1;
    stats.undos += s.undos;
    stats.redos += s.redos;
    stats.dropped_on_rebase += s.dropped_on_rebase;
    stats.resent += s.resent;
    stats.resyncs += s.resyncs;
    stats.diverged += s.diverged;
    stats.late_rejects += s.late_rejects;
    stats.reordered += s.reordered;
    stats.duplicated += s.duplicated;
    stats.duplicates_ignored += s.duplicates_ignored;
    stats.exact_checks += s.exact_checks;
    stats.quiet_pairs += s.quiet_pairs;
    stats.quiet_pair_diffs += s.quiet_pair_diffs;
    stats.rejects += c.rejects;
    stats.dep_rejects += c.dep_rejects;
    stats.ur_accepted += c.ur_accepted;
    stats.i4_checked += c.i4_checked;
    stats.i4_concurrent += c.i4_concurrent;
    stats.i8_checked += c.i8_checked;
}
