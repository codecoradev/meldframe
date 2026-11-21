//! The bounds of a file's rows and the roll (ADR-0023): which rows go when a new one is made, and when it is refused.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_checkpoints::{
    Checkpoint, Kind, Label, MAX_BEFORE, MAX_MANUAL, MAX_MANUAL_PER_ACTOR, MAX_PER_FILE,
    MAX_RUN_START, MAX_RUN_START_PER_ACTOR, NewCheckpoint, Refusal, roll,
};

/// What a store does with the plan: the rows held and the counter, under the one lock.
struct Model {
    held: Vec<Checkpoint>,
    next_n: u32,
    /// Every `n` ever given.
    given: Vec<u32>,
}

impl Model {
    fn new() -> Self {
        Model {
            held: Vec::new(),
            next_n: 1,
            given: Vec::new(),
        }
    }

    fn add(&mut self, new: NewCheckpoint) -> Result<u32, Refusal> {
        let plan = roll(&self.held, self.next_n, &new)?;
        assert!(
            plan.delete
                .iter()
                .all(|n| self.held.iter().any(|c| c.n == *n))
        );
        self.held.retain(|c| !plan.delete.contains(&c.n));
        self.held.push(Checkpoint {
            n: plan.n,
            seq: new.seq(),
            kind: new.kind(),
            label: new.label().cloned(),
            about: new.about().map(str::to_owned),
            by: new.by().map(str::to_owned),
            at_ms: 0,
        });
        self.next_n = plan.next_n;
        assert!(
            !self.given.contains(&plan.n),
            "n {} was given twice",
            plan.n
        );
        assert!(self.given.last().is_none_or(|last| *last < plan.n));
        self.given.push(plan.n);
        Ok(plan.n)
    }

    fn count(&self, f: impl Fn(&Checkpoint) -> bool) -> usize {
        self.held.iter().filter(|c| f(c)).count()
    }

    fn ns(&self, f: impl Fn(&Checkpoint) -> bool) -> Vec<u32> {
        self.held.iter().filter(|c| f(c)).map(|c| c.n).collect()
    }
}

fn manual(by: &str) -> NewCheckpoint {
    NewCheckpoint::by_person(Kind::Manual, 1, Label::parse("x").unwrap(), None, by).unwrap()
}

fn before(kind: Kind) -> NewCheckpoint {
    NewCheckpoint::by_person(kind, 1, None, None, "person:ana").unwrap()
}

fn run(about: &str) -> NewCheckpoint {
    NewCheckpoint::run_start(about, 1).unwrap()
}

fn full(r: Result<u32, Refusal>) -> (usize, &'static str) {
    match r.unwrap_err() {
        Refusal::Full { limit, what } => (limit, what),
        other => panic!("expected Full, got {other:?}"),
    }
}

#[test]
fn a_row_takes_the_next_number_and_a_file_below_its_shares_rolls_nothing() {
    let mut m = Model::new();
    for i in 1..=5 {
        assert_eq!(m.add(manual("person:ana")).unwrap(), i);
    }
    let plan = roll(&m.held, m.next_n, &run("agent:a")).unwrap();
    assert_eq!((plan.n, plan.next_n), (6, 7));
    assert!(plan.delete.is_empty());
}

#[test]
fn a_manual_row_is_refused_at_its_share_and_never_rolled() {
    let mut m = Model::new();
    // Three people: 20 + 20 + 10 reach the share of 50 without any of them reaching their own cap.
    for (by, count) in [("person:a", 20), ("person:b", 20), ("person:c", 10)] {
        for _ in 0..count {
            m.add(manual(by)).unwrap();
        }
    }
    assert_eq!(m.count(|c| c.kind == Kind::Manual), MAX_MANUAL);
    let (limit, what) = full(m.add(manual("person:d")));
    assert_eq!(limit, MAX_MANUAL);
    assert!(what.contains("file"), "{what}");
    assert_eq!(m.count(|c| c.kind == Kind::Manual), MAX_MANUAL);
    // The refusal took no number.
    assert_eq!(m.next_n, 51);
}

#[test]
fn a_manual_row_is_refused_at_the_cap_of_its_actor_and_another_actor_still_creates() {
    let mut m = Model::new();
    for _ in 0..MAX_MANUAL_PER_ACTOR {
        m.add(manual("person:a")).unwrap();
    }
    let (limit, what) = full(m.add(manual("person:a")));
    assert_eq!(limit, MAX_MANUAL_PER_ACTOR);
    assert!(what.contains("actor"), "{what}");
    assert!(m.add(manual("person:b")).is_ok());
}

#[test]
fn the_refusal_has_its_code() {
    let mut m = Model::new();
    for _ in 0..MAX_MANUAL_PER_ACTOR {
        m.add(manual("person:a")).unwrap();
    }
    let r = roll(&m.held, m.next_n, &manual("person:a")).unwrap_err();
    assert_eq!(r.code(), "checkpoint_full");
    assert!(!r.to_string().is_empty());
}

#[test]
fn run_starts_roll_inside_their_kind_at_the_share_oldest_first() {
    let mut m = Model::new();
    // 25 run starts from 5 agents, two or three rows each and none at its own cap.
    for i in 0..MAX_RUN_START {
        m.add(run(&format!("agent:{}", i % 5))).unwrap();
    }
    let oldest = m.ns(|c| c.kind == Kind::RunStart)[0];
    let plan = roll(&m.held, m.next_n, &run("agent:9")).unwrap();
    assert_eq!(plan.delete, vec![oldest]);
    m.add(run("agent:9")).unwrap();
    assert_eq!(m.count(|c| c.kind == Kind::RunStart), MAX_RUN_START);
}

#[test]
fn a_roll_never_drops_a_manual_row_and_never_crosses_kinds() {
    let mut m = Model::new();
    // The oldest rows are manual and before rows, so any roll that took "the oldest of the file" would take them.
    for _ in 0..5 {
        m.add(manual("person:a")).unwrap();
    }
    for i in 0..MAX_BEFORE {
        let kind = if i % 2 == 0 {
            Kind::BeforeRevert
        } else {
            Kind::BeforeRestore
        };
        m.add(before(kind)).unwrap();
    }
    for i in 0..MAX_RUN_START {
        m.add(run(&format!("agent:{}", i % 5))).unwrap();
    }
    let manual_ns = m.ns(|c| c.kind == Kind::Manual);
    let before_ns = m.ns(|c| matches!(c.kind, Kind::BeforeRevert | Kind::BeforeRestore));
    // A flood of run starts and of before rows, far past their shares.
    for i in 0..200 {
        m.add(run(&format!("agent:{}", i % 7))).unwrap();
    }
    assert_eq!(m.ns(|c| c.kind == Kind::Manual), manual_ns);
    assert_eq!(
        m.ns(|c| matches!(c.kind, Kind::BeforeRevert | Kind::BeforeRestore)),
        before_ns
    );
    for i in 0..200 {
        let kind = if i % 3 == 0 {
            Kind::BeforeRestore
        } else {
            Kind::BeforeRevert
        };
        m.add(before(kind)).unwrap();
    }
    assert_eq!(m.ns(|c| c.kind == Kind::Manual), manual_ns);
    assert_eq!(m.count(|c| c.kind == Kind::RunStart), MAX_RUN_START);
    assert_eq!(
        m.count(|c| matches!(c.kind, Kind::BeforeRevert | Kind::BeforeRestore)),
        MAX_BEFORE
    );
}

#[test]
fn the_two_before_kinds_share_one_share_and_the_oldest_of_either_rolls() {
    let mut m = Model::new();
    for i in 0..MAX_BEFORE {
        let kind = if i % 2 == 0 {
            Kind::BeforeRestore
        } else {
            Kind::BeforeRevert
        };
        m.add(before(kind)).unwrap();
    }
    let oldest = m.ns(|c| c.kind != Kind::Manual)[0];
    assert_eq!(
        m.held.iter().find(|c| c.n == oldest).unwrap().kind,
        Kind::BeforeRestore
    );
    let plan = roll(&m.held, m.next_n, &before(Kind::BeforeRevert)).unwrap();
    assert_eq!(plan.delete, vec![oldest]);
}

#[test]
fn a_run_start_at_the_cap_of_its_agent_rolls_that_agents_oldest_row_only() {
    let mut m = Model::new();
    // The other agent's rows are the oldest ones of the kind.
    for _ in 0..3 {
        m.add(run("agent:b")).unwrap();
    }
    for _ in 0..MAX_RUN_START_PER_ACTOR {
        m.add(run("agent:a")).unwrap();
    }
    let a_oldest = m.ns(|c| c.about.as_deref() == Some("agent:a"))[0];
    let plan = roll(&m.held, m.next_n, &run("agent:a")).unwrap();
    assert_eq!(plan.delete, vec![a_oldest]);
    m.add(run("agent:a")).unwrap();
    assert_eq!(
        m.count(|c| c.about.as_deref() == Some("agent:a")),
        MAX_RUN_START_PER_ACTOR
    );
    assert_eq!(m.count(|c| c.about.as_deref() == Some("agent:b")), 3);
}

#[test]
fn an_agent_that_starts_runs_all_day_only_replaces_its_own_rows() {
    let mut m = Model::new();
    for i in 0..5 {
        m.add(run(&format!("agent:b{i}"))).unwrap();
    }
    m.add(manual("person:ana")).unwrap();
    m.add(before(Kind::BeforeRevert)).unwrap();
    let theirs = m.ns(|c| c.about.as_deref().is_some_and(|a| a.starts_with("agent:b")));
    for _ in 0..500 {
        m.add(run("agent:flood")).unwrap();
    }
    assert_eq!(
        m.count(|c| c.about.as_deref() == Some("agent:flood")),
        MAX_RUN_START_PER_ACTOR
    );
    assert_eq!(
        m.ns(|c| c.about.as_deref().is_some_and(|a| a.starts_with("agent:b"))),
        theirs
    );
    // A person's manual create still works.
    assert!(m.add(manual("person:ana")).is_ok());
    assert_eq!(m.count(|c| c.kind == Kind::Manual), 2);
}

#[test]
fn when_the_agent_is_at_its_cap_and_the_share_is_full_one_row_goes_if_that_is_enough() {
    let mut m = Model::new();
    for i in 0..(MAX_RUN_START - MAX_RUN_START_PER_ACTOR) {
        m.add(run(&format!("agent:o{}", i % 4))).unwrap();
    }
    for _ in 0..MAX_RUN_START_PER_ACTOR {
        m.add(run("agent:a")).unwrap();
    }
    assert_eq!(m.count(|c| c.kind == Kind::RunStart), MAX_RUN_START);
    let plan = roll(&m.held, m.next_n, &run("agent:a")).unwrap();
    assert_eq!(
        plan.delete.len(),
        1,
        "the agent's own oldest row makes the room"
    );
    assert_eq!(
        m.held
            .iter()
            .find(|c| c.n == plan.delete[0])
            .unwrap()
            .about
            .as_deref(),
        Some("agent:a")
    );
}

#[test]
fn numbers_are_never_reused_after_a_roll_and_the_counter_is_checked() {
    let mut m = Model::new();
    for i in 0..300 {
        m.add(run(&format!("agent:{}", i % 3))).unwrap();
    }
    assert_eq!(m.given, (1..=300).collect::<Vec<u32>>());
    assert_eq!(m.held.len(), MAX_RUN_START);
    // A counter that is not above every row held would reuse a number: refused, never worked around.
    let top = m.held.iter().map(|c| c.n).max().unwrap();
    for bad in [0, 1, top] {
        assert_eq!(
            roll(&m.held, bad, &run("agent:z")).unwrap_err(),
            Refusal::Counter,
            "{bad}"
        );
    }
    assert!(roll(&m.held, top + 1, &run("agent:z")).is_ok());
    assert_eq!(
        roll(&[], u32::MAX, &run("agent:z")).unwrap_err(),
        Refusal::Counter
    );
    assert_eq!(Refusal::Counter.code(), "unavailable");
}

#[test]
fn a_random_mix_never_breaks_a_share_a_cap_or_the_total() {
    // A fixed generator, so a failure replays.
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut m = Model::new();
    let mut refused = 0;
    for _ in 0..3_000 {
        let r = next();
        let who = format!("{}", r >> 40 & 7);
        let new = match r % 4 {
            0 => manual(&format!("person:{who}")),
            1 => before(Kind::BeforeRevert),
            2 => before(Kind::BeforeRestore),
            _ => run(&format!("agent:{who}")),
        };
        if m.add(new).is_err() {
            refused += 1;
        }
        assert!(m.held.len() <= MAX_PER_FILE);
        assert!(m.count(|c| c.kind == Kind::Manual) <= MAX_MANUAL);
        assert!(m.count(|c| c.kind == Kind::RunStart) <= MAX_RUN_START);
        assert!(
            m.count(|c| matches!(c.kind, Kind::BeforeRevert | Kind::BeforeRestore)) <= MAX_BEFORE
        );
        for a in 0..8 {
            let person = format!("person:{a}");
            let agent = format!("agent:{a}");
            assert!(
                m.count(|c| c.kind == Kind::Manual && c.by.as_deref() == Some(&person))
                    <= MAX_MANUAL_PER_ACTOR
            );
            assert!(
                m.count(|c| c.kind == Kind::RunStart && c.about.as_deref() == Some(&agent))
                    <= MAX_RUN_START_PER_ACTOR
            );
        }
    }
    // Only manual rows are ever refused, and they were: the mix reached both of their bounds.
    assert!(refused > 0);
    assert_eq!(m.given.len() + refused, 3_000);
}
