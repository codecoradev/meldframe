//! The checkpoints in Postgres (ADR-0023): the store passes the cases of the in-memory one, a new process reads the
//! rows back and goes on numbering from where they were, and calls at once never repeat a number or pass a share.

#[path = "../../../../apps/server/tests/rooms_common/mod.rs"]
mod common;

use checkpoint_store::{CheckpointError, CheckpointStore, Made, contract};
use meldframe_checkpoints::{
    Checkpoint, Kind, Label, MAX_MANUAL_PER_ACTOR, MAX_RUN_START, MAX_RUN_START_PER_ACTOR,
    NewCheckpoint,
};
use meldframe_host::{NewOp, Store};
use meldframe_protocol::Command;
use meldframe_server::{PgStore, room_key};
use std::collections::HashSet;
use std::sync::{Arc, Barrier, Mutex};

/// A row points into a file's log, so its file exists: this store makes one op in a file before the first row of it,
/// as the server's call has done by then. The cases use many file names.
#[derive(Debug)]
struct Seeded {
    store: PgStore,
    seen: Mutex<HashSet<String>>,
}

fn seeded(store: PgStore) -> Seeded {
    Seeded {
        store,
        seen: Mutex::default(),
    }
}

impl Seeded {
    fn seed(&self, ws: &str, file: &str) {
        let room = room_key(ws, file);
        // The lock is held until the seed op is written: a thread that finds the room seen must find its op too.
        let mut seen = self.seen.lock().unwrap();
        if seen.insert(room.clone()) {
            let node = serde_json::from_value(
                serde_json::json!({"id": "n", "type": "frame", "parentId": null, "name": "n"}),
            )
            .unwrap();
            let op = NewOp {
                actor: "seed".into(),
                key: "seed".into(),
                command: Command::AddNode { node, index: None },
            };
            self.store.clone().append(&room, op).unwrap();
        }
    }
}

impl CheckpointStore for Seeded {
    fn create(
        &self,
        ws: &str,
        file: &str,
        key: Option<&str>,
        new: &NewCheckpoint,
        at_ms: u64,
    ) -> Result<Made, CheckpointError> {
        self.seed(ws, file);
        self.store.create(ws, file, key, new, at_ms)
    }

    fn list(&self, ws: &str, file: &str) -> Result<Vec<Checkpoint>, CheckpointError> {
        self.store.list(ws, file)
    }
}

macro_rules! case {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            let Some(db) = common::fresh() else {
                eprintln!("SKIPPED: DATABASE_URL is not set");
                return;
            };
            contract::$name(&seeded(db.store()));
        }
    )*};
}

case!(
    numbers_start_at_one_per_file_and_a_list_is_newest_first,
    a_row_is_kept_as_it_was_made,
    a_retried_key_returns_its_row_and_a_different_request_is_key_reused,
    a_key_belongs_to_one_actor_in_one_file_of_one_workspace,
    a_key_that_rolled_out_of_the_file_makes_a_second_row,
    a_run_start_is_unique_per_actor_and_seq,
    a_manual_row_is_refused_at_its_share_and_at_its_actors_cap_and_never_rolled,
    automatic_rows_roll_inside_their_share_and_never_cross_kinds,
    the_before_kinds_roll_the_oldest_of_either,
    an_agent_that_starts_runs_all_day_only_replaces_its_own_rows,
    another_workspaces_file_is_empty,
    input_is_checked_before_anything_is_written,
);

fn manual(by: &str, label: &str) -> NewCheckpoint {
    let label = Label::parse(label).unwrap();
    NewCheckpoint::by_person(Kind::Manual, 1, label, None, by).unwrap()
}

fn run(about: &str, seq: u64) -> NewCheckpoint {
    NewCheckpoint::run_start(about, seq).unwrap()
}

#[test]
fn a_file_with_no_ops_has_nothing_to_checkpoint() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    // The plain store, with no seed op: the row would point at a log that is not there.
    let e = db
        .store()
        .create("ws", "empty", None, &manual("person:ana", "x"), 1)
        .unwrap_err();
    assert_eq!(e.code(), "invalid");
    assert!(!e.retryable());
    assert!(db.store().list("ws", "empty").unwrap().is_empty());
}

#[test]
fn a_new_process_reads_the_rows_back_and_goes_on_numbering_from_where_they_were() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let s = seeded(db.store());
    let label = "two words \u{1F600}";
    s.create("ws", "f", Some("k1"), &manual("person:ana", label), 10)
        .unwrap();
    let revert = NewCheckpoint::by_person(
        Kind::BeforeRevert,
        3,
        None,
        Some("agent:a".into()),
        "person:ana",
    )
    .unwrap();
    s.create("ws", "f", None, &revert, 11).unwrap();
    s.create("ws", "f", None, &run("agent:b", 9), 12).unwrap();
    // Roll a share past its limit, so the counter is ahead of the rows held.
    for i in 0..(MAX_RUN_START + 5) {
        s.create("ws", "f", None, &run("agent:c", 100 + i as u64), 13)
            .unwrap();
    }
    let before = s.list("ws", "f").unwrap();
    let again = db.reconnect();
    assert_eq!(again.list("ws", "f").unwrap(), before);
    let first = before.iter().find(|c| c.kind == Kind::Manual).unwrap();
    assert_eq!(first.label.as_ref().map(Label::as_str), Some(label));
    assert_eq!(
        (first.n, first.by.as_deref(), first.at_ms),
        (1, Some("person:ana"), 10)
    );
    // The key lives in the database too: the same request answers its row, a different one is refused.
    let seeded_again = seeded(again);
    seeded_again
        .seen
        .lock()
        .unwrap()
        .insert(room_key("ws", "f"));
    let top = before[0].n;
    let retry = seeded_again
        .create("ws", "f", Some("k1"), &manual("person:ana", label), 99)
        .unwrap();
    assert!(!retry.created);
    assert_eq!(retry.checkpoint, *first);
    let next = seeded_again
        .create("ws", "f", None, &run("agent:d", 1), 14)
        .unwrap();
    assert_eq!(
        next.checkpoint.n,
        top + 1,
        "the counter went on past the rolled numbers"
    );
}

/// `threads` threads start together and each runs `work(thread)`.
fn at_once<T: Send + 'static>(
    threads: usize,
    work: impl Fn(usize) -> T + Send + Sync + 'static,
) -> Vec<T> {
    let (start, work) = (Arc::new(Barrier::new(threads)), Arc::new(work));
    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let (start, work) = (start.clone(), work.clone());
            std::thread::spawn(move || {
                start.wait();
                work(t)
            })
        })
        .collect();
    handles.into_iter().map(|h| h.join().unwrap()).collect()
}

#[test]
fn eight_threads_that_take_numbers_together_get_distinct_numbers_without_holes() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = Arc::new(seeded(db.store()));
    // The file exists before the race, so the threads race for numbers and not for the seed op.
    store
        .create("ws", "f", None, &manual("person:seed", "s"), 1)
        .unwrap();
    let s = store.clone();
    let made = at_once(8, move |t| {
        // Six manual rows each stays inside the cap of the actor (20) and the share of the file (50 with the seed).
        (0..6)
            .map(|i| {
                let who = format!("person:{t}");
                let new = manual(&who, &format!("{t}-{i}"));
                s.create("ws", "f", Some(&format!("k{i}")), &new, 2)
                    .unwrap()
                    .checkpoint
                    .n
            })
            .collect::<Vec<_>>()
    });
    let mut ns: Vec<u32> = made.iter().flatten().copied().collect();
    ns.sort_unstable();
    assert_eq!(
        ns,
        (2..=49).collect::<Vec<u32>>(),
        "every number once, none skipped"
    );
    for mine in &made {
        assert!(
            mine.windows(2).all(|w| w[0] < w[1]),
            "a thread's second call comes after its first"
        );
    }
}

#[test]
fn run_starts_that_race_roll_inside_their_share_and_their_agents_cap() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = Arc::new(seeded(db.store()));
    store
        .create("ws", "f", None, &manual("person:ana", "keep"), 1)
        .unwrap();
    let s = store.clone();
    // Eight agents, forty run starts each, all at once: every call is made (an automatic row is never refused).
    let numbers = at_once(8, move |t| {
        (0..40)
            .map(|i| {
                let about = format!("agent:{t}");
                s.create("ws", "f", None, &run(&about, i), 2)
                    .unwrap()
                    .checkpoint
                    .n
            })
            .collect::<Vec<_>>()
    });
    let mut all: Vec<u32> = numbers.iter().flatten().copied().collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 8 * 40, "no number was given twice");
    let rows = store.list("ws", "f").unwrap();
    let run_rows = rows.iter().filter(|c| c.kind == Kind::RunStart).count();
    assert_eq!(run_rows, MAX_RUN_START, "the share holds");
    assert!(
        rows.iter().any(|c| c.kind == Kind::Manual),
        "the manual row was never rolled"
    );
    for t in 0..8 {
        let mine = rows
            .iter()
            .filter(|c| c.about.as_deref() == Some(&format!("agent:{t}")))
            .count();
        assert!(mine <= MAX_RUN_START_PER_ACTOR, "agent {t} holds {mine}");
    }
}

#[test]
fn creates_of_one_actor_at_once_never_pass_its_cap() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = Arc::new(seeded(db.store()));
    store
        .create("ws", "f", None, &manual("person:seed", "s"), 1)
        .unwrap();
    let s = store.clone();
    let results = at_once(8, move |t| {
        (0..8)
            .map(|i| {
                s.create(
                    "ws",
                    "f",
                    Some(&format!("k{t}-{i}")),
                    &manual("person:a", "x"),
                    2,
                )
                .is_ok()
            })
            .filter(|ok| *ok)
            .count()
    });
    assert_eq!(
        results.iter().sum::<usize>(),
        MAX_MANUAL_PER_ACTOR,
        "exactly the cap was made"
    );
    let rows = store.list("ws", "f").unwrap();
    assert_eq!(
        rows.iter()
            .filter(|c| c.by.as_deref() == Some("person:a"))
            .count(),
        MAX_MANUAL_PER_ACTOR
    );
}

#[test]
fn the_same_request_at_once_is_one_row() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = Arc::new(seeded(db.store()));
    store
        .create("ws", "f", None, &manual("person:seed", "s"), 1)
        .unwrap();
    let s = store.clone();
    let made = at_once(8, move |_| {
        let m = s
            .create("ws", "f", Some("same"), &manual("person:a", "x"), 2)
            .unwrap();
        (m.created, m.checkpoint.n)
    });
    assert_eq!(made.iter().filter(|(created, _)| *created).count(), 1);
    assert!(made.iter().all(|(_, n)| *n == 2));
    // And a run start at once is one row too.
    let s = store.clone();
    let made = at_once(8, move |_| {
        s.create("ws", "f", None, &run("agent:a", 5), 3)
            .unwrap()
            .created
    });
    assert_eq!(made.iter().filter(|c| **c).count(), 1);
}
