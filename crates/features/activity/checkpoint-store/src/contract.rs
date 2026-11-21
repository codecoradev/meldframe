//! The cases every [`CheckpointStore`] passes: the in-memory store and the Postgres one run the same functions, each on
//! an empty store. A case panics when the store breaks a rule.
//!
//! One rule differs by adapter: a row points into a file's log, so Postgres refuses one for a file that has no ops yet
//! and the in-memory store, which has no files, accepts any file name. The cases run on files that exist in both.

use crate::{CheckpointError, CheckpointStore, MAX_KEY};
use meldframe_checkpoints::{
    Checkpoint, Kind, Label, MAX_BEFORE, MAX_MANUAL, MAX_MANUAL_PER_ACTOR, MAX_RUN_START,
    MAX_RUN_START_PER_ACTOR, NewCheckpoint,
};

const AT: u64 = 1_700_000_000_000;

fn manual(by: &str, seq: u64, label: &str) -> NewCheckpoint {
    let label = Label::parse(label).unwrap();
    NewCheckpoint::by_person(Kind::Manual, seq, label, None, by).unwrap()
}

fn before(kind: Kind, seq: u64) -> NewCheckpoint {
    NewCheckpoint::by_person(kind, seq, None, None, "person:ana").unwrap()
}

fn run(about: &str, seq: u64) -> NewCheckpoint {
    NewCheckpoint::run_start(about, seq).unwrap()
}

fn make(s: &dyn CheckpointStore, ws: &str, file: &str, new: &NewCheckpoint) -> Checkpoint {
    s.create(ws, file, None, new, AT).unwrap().checkpoint
}

fn all(s: &dyn CheckpointStore, ws: &str, file: &str) -> Vec<Checkpoint> {
    s.list(ws, file).unwrap()
}

fn ns(rows: &[Checkpoint], f: impl Fn(&Checkpoint) -> bool) -> Vec<u32> {
    rows.iter().filter(|c| f(c)).map(|c| c.n).collect()
}

fn refused(r: Result<crate::Made, CheckpointError>) -> CheckpointError {
    r.unwrap_err()
}

pub fn numbers_start_at_one_per_file_and_a_list_is_newest_first(s: &dyn CheckpointStore) {
    assert!(all(s, "ws", "f").is_empty(), "a file with none");
    let made: Vec<u32> = (1..=3)
        .map(|i| make(s, "ws", "f", &manual("person:ana", i, "x")).n)
        .collect();
    assert_eq!(made, [1, 2, 3]);
    // Another file, and another workspace with the same file name, start at 1 and see nothing of it.
    assert_eq!(make(s, "ws", "g", &manual("person:ana", 1, "x")).n, 1);
    assert_eq!(make(s, "ws2", "f", &manual("person:ana", 1, "x")).n, 1);
    assert_eq!(ns(&all(s, "ws", "f"), |_| true), [3, 2, 1]);
    assert_eq!(all(s, "ws2", "f").len(), 1);
    assert!(all(s, "ws2", "g").is_empty());
    // The key is not the two names glued together: ws "a/b" file "c" is not ws "a" file "b/c".
    make(s, "a/b", "c", &manual("person:ana", 1, "x"));
    assert!(all(s, "a", "b/c").is_empty());
}

pub fn a_row_is_kept_as_it_was_made(s: &dyn CheckpointStore) {
    let made = s
        .create(
            "ws",
            "f",
            Some("k1"),
            &manual("person:ana", 7, "before the logo"),
            AT,
        )
        .unwrap();
    assert!(made.created && made.rolled.is_empty());
    let want = Checkpoint {
        n: 1,
        seq: 7,
        kind: Kind::Manual,
        label: Label::parse("before the logo").unwrap(),
        about: None,
        by: Some("person:ana".into()),
        at_ms: AT,
    };
    assert_eq!(made.checkpoint, want);
    let revert = NewCheckpoint::by_person(
        Kind::BeforeRevert,
        9,
        None,
        Some("agent:a".into()),
        "person:bo",
    );
    let revert = revert.unwrap();
    s.create("ws", "f", None, &revert, AT + 1).unwrap();
    s.create("ws", "f", None, &run("agent:c", 4), AT + 2)
        .unwrap();
    let rows = all(s, "ws", "f");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].kind, Kind::RunStart);
    assert_eq!(
        (
            rows[0].about.as_deref(),
            rows[0].by.as_deref(),
            rows[0].at_ms
        ),
        (Some("agent:c"), None, AT + 2)
    );
    assert_eq!(
        (rows[1].about.as_deref(), rows[1].by.as_deref()),
        (Some("agent:a"), Some("person:bo"))
    );
    assert_eq!(rows[2], want);
}

pub fn a_retried_key_returns_its_row_and_a_different_request_is_key_reused(
    s: &dyn CheckpointStore,
) {
    let first = s
        .create("ws", "f", Some("k"), &manual("person:ana", 5, "v1"), AT)
        .unwrap();
    assert!(first.created);
    // The same request again: the row it made, no number taken, nothing rolled, even a later clock.
    let again = s
        .create(
            "ws",
            "f",
            Some("k"),
            &manual("person:ana", 5, "v1"),
            AT + 99,
        )
        .unwrap();
    assert!(!again.created && again.rolled.is_empty());
    assert_eq!(again.checkpoint, first.checkpoint);
    assert_eq!(all(s, "ws", "f").len(), 1);
    // The same key with anything else is refused, with the first row's number, and writes nothing.
    let others = [
        manual("person:ana", 6, "v1"),
        manual("person:ana", 5, "v2"),
        manual("person:ana", 5, ""),
        before(Kind::BeforeRestore, 5),
    ];
    for other in &others {
        let e = refused(s.create("ws", "f", Some("k"), other, AT));
        assert_eq!(e, CheckpointError::KeyReused(1));
        assert_eq!(e.code(), "key_reused");
    }
    assert_eq!(all(s, "ws", "f").len(), 1);
    // The next row still takes the next number: the refusals took none.
    assert_eq!(make(s, "ws", "f", &manual("person:ana", 5, "v3")).n, 2);
}

pub fn a_key_belongs_to_one_actor_in_one_file_of_one_workspace(s: &dyn CheckpointStore) {
    let by = |who: &str| manual(who, 1, "x");
    let a = s
        .create("ws", "f", Some("k"), &by("person:ana"), AT)
        .unwrap();
    let b = s
        .create("ws", "f", Some("k"), &by("person:bo"), AT)
        .unwrap();
    let c = s
        .create("ws", "g", Some("k"), &by("person:ana"), AT)
        .unwrap();
    let d = s
        .create("ws2", "f", Some("k"), &by("person:ana"), AT)
        .unwrap();
    assert!([&a, &b, &c, &d].iter().all(|m| m.created));
    assert_eq!((a.checkpoint.n, b.checkpoint.n), (1, 2));
    assert_eq!((c.checkpoint.n, d.checkpoint.n), (1, 1));
}

pub fn a_key_that_rolled_out_of_the_file_makes_a_second_row(s: &dyn CheckpointStore) {
    let first = s
        .create("ws", "f", Some("k"), &before(Kind::BeforeRestore, 1), AT)
        .unwrap();
    for i in 0..MAX_BEFORE {
        make(s, "ws", "f", &before(Kind::BeforeRevert, i as u64));
    }
    assert!(
        !all(s, "ws", "f").iter().any(|c| c.n == first.checkpoint.n),
        "it rolled out"
    );
    // The key bounds a retry in time, not for ever.
    let again = s
        .create("ws", "f", Some("k"), &before(Kind::BeforeRestore, 1), AT)
        .unwrap();
    assert!(again.created);
    assert!(again.checkpoint.n > first.checkpoint.n);
}

pub fn a_run_start_is_unique_per_actor_and_seq(s: &dyn CheckpointStore) {
    let first = s.create("ws", "f", None, &run("agent:a", 10), AT).unwrap();
    assert!(first.created);
    let again = s
        .create("ws", "f", None, &run("agent:a", 10), AT + 5)
        .unwrap();
    assert!(
        !again.created,
        "a repeated call or a retried sink makes no second row"
    );
    assert_eq!(again.checkpoint, first.checkpoint);
    assert!(
        s.create("ws", "f", None, &run("agent:a", 11), AT)
            .unwrap()
            .created
    );
    assert!(
        s.create("ws", "f", None, &run("agent:b", 10), AT)
            .unwrap()
            .created
    );
    assert!(
        s.create("ws", "g", None, &run("agent:a", 10), AT)
            .unwrap()
            .created
    );
    assert_eq!(ns(&all(s, "ws", "f"), |_| true), [3, 2, 1]);
}

pub fn a_manual_row_is_refused_at_its_share_and_at_its_actors_cap_and_never_rolled(
    s: &dyn CheckpointStore,
) {
    for _ in 0..MAX_MANUAL_PER_ACTOR {
        make(s, "ws", "f", &manual("person:a", 1, "x"));
    }
    let e = refused(s.create("ws", "f", None, &manual("person:a", 1, "x"), AT));
    assert_eq!(e.code(), "checkpoint_full");
    assert!(matches!(e, CheckpointError::Full { limit, .. } if limit == MAX_MANUAL_PER_ACTOR));
    // Others fill the share: 20 + 20 + 10 is the 50 of the file.
    for (by, count) in [("person:b", 20), ("person:c", 10)] {
        for _ in 0..count {
            make(s, "ws", "f", &manual(by, 1, "x"));
        }
    }
    let held = ns(&all(s, "ws", "f"), |c| c.kind == Kind::Manual);
    assert_eq!(held.len(), MAX_MANUAL);
    let e = refused(s.create("ws", "f", None, &manual("person:d", 1, "x"), AT));
    assert!(matches!(e, CheckpointError::Full { limit, .. } if limit == MAX_MANUAL));
    // The automatic kinds go on, and never take a manual row's place, even for a flood.
    for i in 0..200 {
        make(s, "ws", "f", &run(&format!("agent:{}", i % 9), i));
        make(s, "ws", "f", &before(Kind::BeforeRevert, i));
    }
    assert_eq!(ns(&all(s, "ws", "f"), |c| c.kind == Kind::Manual), held);
    // Another file of the workspace is not touched by any of it.
    assert!(
        s.create("ws", "g", None, &manual("person:a", 1, "x"), AT)
            .is_ok()
    );
}

pub fn automatic_rows_roll_inside_their_share_and_never_cross_kinds(s: &dyn CheckpointStore) {
    make(s, "ws", "f", &manual("person:a", 1, "x"));
    for i in 0..MAX_BEFORE {
        let kind = if i % 2 == 0 {
            Kind::BeforeRestore
        } else {
            Kind::BeforeRevert
        };
        make(s, "ws", "f", &before(kind, i as u64));
    }
    for i in 0..MAX_RUN_START {
        make(s, "ws", "f", &run(&format!("agent:{}", i % 5), i as u64));
    }
    let rows = all(s, "ws", "f");
    let manual_ns = ns(&rows, |c| c.kind == Kind::Manual);
    let before_ns = ns(&rows, |c| c.kind.share() == Kind::BeforeRevert.share());
    let oldest_run = *ns(&rows, |c| c.kind == Kind::RunStart).last().unwrap();
    // One more run start: the oldest run start goes, and only it.
    let made = s.create("ws", "f", None, &run("agent:9", 99), AT).unwrap();
    assert_eq!(made.rolled, [oldest_run]);
    let rows = all(s, "ws", "f");
    assert_eq!(ns(&rows, |c| c.kind == Kind::Manual), manual_ns);
    assert_eq!(
        ns(&rows, |c| c.kind.share() == Kind::BeforeRevert.share()),
        before_ns
    );
    assert_eq!(ns(&rows, |c| c.kind == Kind::RunStart).len(), MAX_RUN_START);
    // Past a roll a number is not given twice: every number is one above the one before.
    let top = rows[0].n;
    assert_eq!(top, 1 + MAX_BEFORE as u32 + MAX_RUN_START as u32 + 1);
    for i in 0..300 {
        assert!(make(s, "ws", "f", &run("agent:z", u64::from(top) + i)).n > top);
    }
    let next = make(s, "ws", "f", &before(Kind::BeforeRevert, 0));
    assert_eq!(next.n, top + 300 + 1);
}

pub fn the_before_kinds_roll_the_oldest_of_either(s: &dyn CheckpointStore) {
    for i in 0..MAX_BEFORE {
        let kind = if i % 2 == 0 {
            Kind::BeforeRestore
        } else {
            Kind::BeforeRevert
        };
        make(s, "ws", "f", &before(kind, i as u64));
    }
    let made = s
        .create("ws", "f", None, &before(Kind::BeforeRevert, 0), AT)
        .unwrap();
    assert_eq!(made.rolled, [1]);
    let made = s
        .create("ws", "f", None, &before(Kind::BeforeRestore, 0), AT)
        .unwrap();
    assert_eq!(made.rolled, [2]);
}

pub fn an_agent_that_starts_runs_all_day_only_replaces_its_own_rows(s: &dyn CheckpointStore) {
    for i in 0..5 {
        make(s, "ws", "f", &run(&format!("agent:b{i}"), i));
    }
    make(s, "ws", "f", &manual("person:ana", 1, "keep"));
    let theirs = ns(&all(s, "ws", "f"), |c| {
        c.about.as_deref().is_some_and(|a| a.starts_with("agent:b"))
    });
    for i in 0..300 {
        make(s, "ws", "f", &run("agent:flood", 100 + i));
    }
    let rows = all(s, "ws", "f");
    assert_eq!(
        ns(&rows, |c| c.about.as_deref() == Some("agent:flood")).len(),
        MAX_RUN_START_PER_ACTOR
    );
    assert_eq!(
        ns(&rows, |c| c
            .about
            .as_deref()
            .is_some_and(|a| a.starts_with("agent:b"))),
        theirs
    );
    // A refused create by an actor at its cap changes nothing, and a person's own create still works.
    assert!(
        s.create("ws", "f", None, &manual("person:ana", 2, "again"), AT)
            .is_ok()
    );
    assert_eq!(ns(&all(s, "ws", "f"), |c| c.kind == Kind::Manual).len(), 2);
}

pub fn another_workspaces_file_is_empty(s: &dyn CheckpointStore) {
    make(s, "ws", "f", &manual("person:ana", 1, "x"));
    make(s, "ws", "f", &run("agent:a", 1));
    assert!(all(s, "ws2", "f").is_empty());
    assert!(all(s, "ws", "nope").is_empty());
    // The same key in the other workspace is a row of its own.
    let a = s
        .create("ws", "f", Some("k"), &manual("person:ana", 1, "x"), AT)
        .unwrap();
    let b = s
        .create("ws2", "f", Some("k"), &manual("person:ana", 1, "x"), AT)
        .unwrap();
    assert!(a.created && b.created);
    assert_eq!(b.checkpoint.n, 1);
}

pub fn input_is_checked_before_anything_is_written(s: &dyn CheckpointStore) {
    let long = "k".repeat(MAX_KEY + 1);
    for key in ["", &long, "a\nb", "k\u{200B}", "\u{202E}k"] {
        let e = refused(s.create("ws", "f", Some(key), &manual("person:ana", 1, "x"), AT));
        assert_eq!(e.code(), "invalid", "{key:?}");
        assert!(!e.retryable());
    }
    // The longest key is fine, and so are other kinds with one.
    assert!(
        s.create(
            "ws",
            "f",
            Some(&"k".repeat(MAX_KEY)),
            &manual("person:ana", 1, "x"),
            AT
        )
        .is_ok()
    );
    // A run start is the server's and has no key: it is deduplicated by (actor, seq).
    let e = refused(s.create("ws", "f", Some("k"), &run("agent:a", 1), AT));
    assert_eq!(e.code(), "invalid");
    // Nothing was written and no number was taken by a refusal.
    assert_eq!(all(s, "ws", "f").len(), 1);
    assert_eq!(make(s, "ws", "f", &manual("person:ana", 1, "y")).n, 2);
}
