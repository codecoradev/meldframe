//! The batch headers in Postgres (ADR-0021): the store passes the cases of the in-memory one, a new process reads
//! the headers back, and calls at once never repeat a number, a `rev` or go past a cap.

#[path = "../../meldframe-server-tests/tests/common/mod.rs"]
mod common;

use activity_store::{ActivityError, BatchStore, Caps, Page, Query, Recorded, Reviewed, contract};
use meldframe_activity::{NewBatch, Review};
use meldframe_host::{NewOp, Store};
use meldframe_protocol::Command;
use meldframe_server::{PgStore, room_key};
use std::collections::HashSet;
use std::sync::{Arc, Barrier, Mutex};

/// A header is written after its ops, so its file exists: this store makes one op in a file before the first header
/// of it, as the server's call has done by then. The cases use many file names.
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

impl BatchStore for Seeded {
    fn record(&self, ws: &str, file: &str, b: &NewBatch) -> Result<Recorded, ActivityError> {
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
        drop(seen);
        self.store.record(ws, file, b)
    }

    fn list(&self, ws: &str, file: &str, q: &Query) -> Result<Page, ActivityError> {
        self.store.list(ws, file, q)
    }

    fn set_review(
        &self,
        ws: &str,
        file: &str,
        n: u32,
        s: Review,
        by: &str,
        at: u64,
    ) -> Result<Reviewed, ActivityError> {
        self.store.set_review(ws, file, n, s, by, at)
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
    numbers_start_at_one_per_file_and_are_never_reused,
    a_batch_is_one_header_per_actor_and_key_prefix,
    rev_rises_on_every_change_and_not_on_a_no_op,
    an_update_never_lowers_and_may_raise_what_it_keeps,
    calls_that_know_what_the_log_holds_count_each_op_once,
    commands_never_fall_below_written_across_the_parts_of_an_import,
    the_first_note_wins_and_the_answer_says_which,
    a_review_is_set_by_a_person_and_survives_a_later_record,
    a_review_of_a_missing_batch_or_one_of_another_workspace_is_not_found,
    the_unreviewed_count_is_every_state_but_accepted,
    a_list_is_newest_first_with_before_after_and_has_more,
    input_is_checked_before_anything_is_written,
);

#[test]
fn the_caps_refuse_a_new_header_and_never_an_update() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    contract::the_caps_refuse_a_new_header_and_never_an_update(&seeded(
        db.store().with_batch_caps(contract::small_caps()),
    ));
}

#[test]
fn the_default_caps_are_the_bounds_of_the_adr() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let caps = Caps::default();
    assert_eq!(
        (caps.per_actor, caps.per_file, caps.per_workspace),
        (2_000, 20_000, 200_000)
    );
    drop(db);
}

#[test]
fn a_new_process_reads_the_headers_back_as_they_were_left() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let s = seeded(db.store());
    let mut b = contract::batch("agent:a", "k1", 4, 2);
    b.note = Some(meldframe_activity::Note::parse("two\nlines \u{1F600}").unwrap());
    s.record("ws", "f", &b).unwrap();
    s.record("ws", "f", &contract::batch("agent:a", "k2", 1, 1))
        .unwrap();
    s.set_review("ws", "f", 1, Review::NeedsReview, "person:p", 77)
        .unwrap();
    let again = db.reconnect();
    let q = Query {
        before: None,
        after: None,
        limit: 10,
    };
    let page = again.list("ws", "f", &q).unwrap();
    assert_eq!(page.headers.iter().map(|h| h.n).collect::<Vec<_>>(), [2, 1]);
    let first = &page.headers[1];
    assert_eq!(
        first.note.as_ref().map(|n| n.as_str()),
        Some("two\nlines \u{1F600}")
    );
    assert_eq!(
        (
            first.review,
            first.reviewed_by.as_deref(),
            first.reviewed_at_ms
        ),
        (Review::NeedsReview, Some("person:p"), Some(77))
    );
    assert_eq!((first.commands, first.written, first.rev), (4, 2, 3));
    assert_eq!(page.unreviewed, 2);
    // The counters live in the database too: the next number and rev go on from where they were.
    let next = seeded(again)
        .record("ws", "f", &contract::batch("agent:a", "k3", 1, 1))
        .unwrap();
    assert_eq!((next.header.n, next.header.rev), (3, 4));
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
fn eight_threads_that_take_numbers_together_get_distinct_numbers_and_revs_without_holes() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = Arc::new(seeded(db.store()));
    // The file exists before the race, so the threads race for numbers and not for the seed op.
    store
        .record("ws", "f", &contract::batch("agent:seed", "s", 1, 1))
        .unwrap();
    let s = store.clone();
    let made = at_once(8, move |t| {
        (0..25)
            .map(|i| {
                s.record(
                    "ws",
                    "f",
                    &contract::batch(&format!("agent:{t}"), &format!("k{i}"), 1, 1),
                )
                .unwrap()
                .header
            })
            .collect::<Vec<_>>()
    });
    let mut ns: Vec<u32> = made.iter().flatten().map(|h| h.n).collect();
    let mut revs: Vec<u64> = made.iter().flatten().map(|h| h.rev).collect();
    ns.sort_unstable();
    revs.sort_unstable();
    assert_eq!(
        ns,
        (2..=201).collect::<Vec<u32>>(),
        "every number once, none skipped"
    );
    assert_eq!(
        revs,
        (2..=201).collect::<Vec<u64>>(),
        "every rev once, none skipped"
    );
    // Within one thread the numbers rise: a thread's second call comes after its first.
    for mine in &made {
        assert!(mine.windows(2).all(|w| w[0].n < w[1].n));
    }
}

#[test]
fn the_same_batch_recorded_at_once_is_one_header() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = Arc::new(seeded(db.store()));
    store
        .record("ws", "f", &contract::batch("agent:seed", "s", 1, 1))
        .unwrap();
    let s = store.clone();
    let made = at_once(8, move |_| {
        s.record("ws", "f", &contract::batch("agent:a", "same", 3, 1))
            .unwrap()
    });
    assert_eq!(
        made.iter().filter(|r| r.created).count(),
        1,
        "one call made it"
    );
    let header = made.iter().find(|r| r.created).unwrap().header.n;
    assert!(made.iter().all(|r| r.header.n == header));
    let page = store
        .list(
            "ws",
            "f",
            &Query {
                before: None,
                after: None,
                limit: 10,
            },
        )
        .unwrap();
    assert_eq!(page.headers.len(), 2);
    // Eight calls each wrote one op of three: the count is what they wrote, capped by nothing but the bound.
    assert_eq!(page.headers[0].written, 8);
}

#[test]
fn reviews_set_at_once_end_with_the_row_of_the_highest_rev() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = Arc::new(seeded(db.store()));
    store
        .record("ws", "f", &contract::batch("agent:a", "k", 1, 1))
        .unwrap();
    let s = store.clone();
    let states = [Review::Accepted, Review::NeedsReview];
    let done = at_once(8, move |t| {
        s.set_review(
            "ws",
            "f",
            1,
            states[t % 2],
            &format!("person:{t}"),
            t as u64,
        )
        .unwrap()
    });
    let last = done.iter().max_by_key(|r| r.header.rev).unwrap();
    let now = store
        .list(
            "ws",
            "f",
            &Query {
                before: None,
                after: None,
                limit: 1,
            },
        )
        .unwrap();
    assert_eq!(
        now.headers[0], last.header,
        "the stored row is the one of the highest rev"
    );
    let mut revs: Vec<u64> = done
        .iter()
        .filter(|r| r.changed)
        .map(|r| r.header.rev)
        .collect();
    revs.sort_unstable();
    revs.dedup();
    assert_eq!(
        revs.len(),
        done.iter().filter(|r| r.changed).count(),
        "no rev was given twice"
    );
}

#[test]
fn the_caps_hold_when_calls_run_at_once() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let caps = Caps {
        per_actor: 100,
        per_file: 5,
        per_workspace: 8,
    };
    let store = Arc::new(seeded(db.store().with_batch_caps(caps)));
    store
        .record("ws", "f", &contract::batch("agent:seed", "s", 1, 1))
        .unwrap();
    let s = store.clone();
    let results = at_once(8, move |t| {
        s.record(
            "ws",
            "f",
            &contract::batch(&format!("agent:{t}"), "k", 1, 1),
        )
    });
    let ok = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(ok, 4, "the seed and four more fill the file of five");
    assert!(
        results
            .iter()
            .all(|r| r.is_ok() || matches!(r, Err(ActivityError::Full { limit: 5, .. })))
    );
    // The refused ones took no number: the headers are 1 to 5.
    let page = store
        .list(
            "ws",
            "f",
            &Query {
                before: None,
                after: None,
                limit: 10,
            },
        )
        .unwrap();
    assert_eq!(
        page.headers.iter().map(|h| h.n).collect::<Vec<_>>(),
        [5, 4, 3, 2, 1]
    );
    // Two files of one workspace race for the 8 of the workspace: three more fit, and only three are made.
    let s = store.clone();
    let results = at_once(8, move |t| {
        s.record(
            "ws",
            "g",
            &contract::batch(&format!("agent:{t}"), "k", 1, 1),
        )
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 3);
    assert!(
        results
            .iter()
            .all(|r| r.is_ok() || matches!(r, Err(ActivityError::Full { limit: 8, .. })))
    );
}

#[test]
fn a_header_for_a_file_with_no_ops_is_refused_as_a_permanent_error() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    // The store itself, not the seeded one: a header is written after its ops, so the file must exist.
    let r = db
        .store()
        .record("ws", "nope", &contract::batch("agent:a", "k", 1, 1));
    let e = r.expect_err("no file");
    assert!(matches!(e, ActivityError::Invalid(_)), "{e:?}");
    assert!(!e.retryable());
    assert!(!e.to_string().contains("sqlx"), "{e}");
}

/// The unreviewed count the headers say, counted from the pages.
fn recount(s: &dyn BatchStore, file: &str) -> u32 {
    let (mut before, mut open) = (None, 0);
    loop {
        let page = s
            .list(
                "ws",
                file,
                &Query {
                    before,
                    after: None,
                    limit: 100,
                },
            )
            .unwrap();
        open += page.headers.iter().filter(|h| h.review.is_open()).count() as u32;
        match (page.has_more, page.headers.last()) {
            (true, Some(last)) => before = Some(last.n),
            _ => return open,
        }
    }
}

#[test]
fn the_counter_of_open_headers_follows_creations_and_reviews_made_at_once() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = Arc::new(seeded(db.store()));
    store
        .record("ws", "f", &contract::batch("agent:seed", "s", 1, 1))
        .unwrap();
    let s = store.clone();
    // Each thread makes ten headers and, as it goes, accepts or flags the one before and clears the one before that.
    let seen = at_once(8, move |t| {
        let mut last = Vec::new();
        for i in 0..10 {
            let n = s
                .record(
                    "ws",
                    "f",
                    &contract::batch(&format!("agent:{t}"), &format!("k{i}"), 1, 1),
                )
                .unwrap()
                .header
                .n;
            last.push(n);
            let state = [Review::Accepted, Review::NeedsReview, Review::Unreviewed][i % 3];
            s.set_review("ws", "f", last[i.saturating_sub(1)], state, "person:p", 1)
                .unwrap();
            s.set_review(
                "ws",
                "f",
                last[i.saturating_sub(2)],
                Review::Accepted,
                "person:q",
                2,
            )
            .unwrap();
        }
        last
    });
    assert_eq!(seen.len(), 8);
    let page = store
        .list(
            "ws",
            "f",
            &Query {
                before: None,
                after: None,
                limit: 1,
            },
        )
        .unwrap();
    assert_eq!(
        page.unreviewed,
        recount(&*store, "f"),
        "the counter is the count of the rows"
    );
    // The answer of the last call of each kind agrees with it, and a no-op review leaves it as it is.
    let again = store
        .set_review("ws", "f", 2, Review::Accepted, "person:p", 3)
        .unwrap();
    assert_eq!(again.unreviewed, recount(&*store, "f"));
}
