//! The cases every [`BatchStore`] passes: the in-memory store and the Postgres one run the same functions, each on an
//! empty store. A case panics when the store breaks a rule. The cases that need caps take a store built with
//! [`small_caps`].
//!
//! One rule differs by adapter: a header is written after its ops, so Postgres refuses one for a file that has no
//! ops yet (`Invalid`, not retryable) and the in-memory store, which has no files, accepts any file name. The cases
//! run on files that exist in both.

use crate::{ActivityError, BatchStore, Caps, NoteOutcome, Query, Recorded};
use meldframe_activity::{
    ACTOR_HEADERS, FILE_HEADERS, Header, MAX_ACTOR, MAX_COUNT, MAX_KEY_PREFIX, MAX_PAGE, MAX_TOOL,
    NewBatch, Note, Review, WORKSPACE_HEADERS,
};

/// The caps the cap cases need: 3 headers of an actor, 5 of a file, 8 of a workspace.
pub fn small_caps() -> Caps {
    Caps {
        per_actor: 3,
        per_file: 5,
        per_workspace: 8,
    }
}

/// A batch of `actor` with key prefix `prefix`, of `commands` commands of which `written` were written by this call.
pub fn batch(actor: &str, prefix: &str, commands: u32, written: u32) -> NewBatch {
    NewBatch {
        actor: actor.to_owned(),
        key_prefix: prefix.to_owned(),
        tool: "apply_commands".to_owned(),
        commands,
        written,
        known: None,
        atomic: true,
        first_seq: 10,
        last_seq: 10 + u64::from(written),
        at_ms: 1_000,
        note: None,
    }
}

fn with_note(mut b: NewBatch, note: &str) -> NewBatch {
    b.note = Some(Note::parse(note).unwrap());
    b
}

fn rec(s: &dyn BatchStore, ws: &str, file: &str, b: &NewBatch) -> Recorded {
    s.record(ws, file, b).unwrap()
}

fn all(s: &dyn BatchStore, ws: &str, file: &str) -> Vec<Header> {
    let q = Query {
        before: None,
        after: None,
        limit: MAX_PAGE,
    };
    s.list(ws, file, &q).unwrap().headers
}

fn numbers(h: &[Header]) -> Vec<u32> {
    h.iter().map(|h| h.n).collect()
}

pub fn numbers_start_at_one_per_file_and_are_never_reused(s: &dyn BatchStore) {
    assert!(all(s, "ws", "f").is_empty(), "a file with none");
    let made: Vec<u32> = ["a", "b", "c"]
        .iter()
        .map(|p| rec(s, "ws", "f", &batch("agent:x", p, 1, 1)).header.n)
        .collect();
    assert_eq!(made, [1, 2, 3]);
    // Another file, and another workspace with the same file name, start at 1 and see nothing of it.
    assert_eq!(rec(s, "ws", "g", &batch("agent:x", "a", 1, 1)).header.n, 1);
    assert_eq!(rec(s, "ws2", "f", &batch("agent:x", "a", 1, 1)).header.n, 1);
    assert_eq!(numbers(&all(s, "ws", "f")), [3, 2, 1]);
    assert_eq!(all(s, "ws2", "f").len(), 1);
    assert!(all(s, "ws2", "g").is_empty());
    // The key is not the two names glued together: ws "a/b" file "c" is not ws "a" file "b/c".
    rec(s, "a/b", "c", &batch("agent:x", "p", 1, 1));
    assert!(all(s, "a", "b/c").is_empty());
}

pub fn a_batch_is_one_header_per_actor_and_key_prefix(s: &dyn BatchStore) {
    let first = rec(s, "ws", "f", &batch("agent:a", "k1", 2, 2));
    assert!(first.created && first.changed);
    let again = rec(s, "ws", "f", &batch("agent:a", "k1", 2, 0));
    assert!(!again.created);
    assert_eq!(again.header.n, first.header.n);
    // The same prefix of another actor, and another prefix of the same actor, are batches of their own.
    assert_eq!(rec(s, "ws", "f", &batch("agent:b", "k1", 1, 1)).header.n, 2);
    assert_eq!(rec(s, "ws", "f", &batch("agent:a", "k2", 1, 1)).header.n, 3);
    assert_eq!(all(s, "ws", "f").len(), 3);
}

pub fn rev_rises_on_every_change_and_not_on_a_no_op(s: &dyn BatchStore) {
    let a = rec(s, "ws", "f", &batch("agent:a", "k", 5, 2));
    assert_eq!(a.header.rev, 1);
    // A replay that adds nothing changes nothing: no new rev, the header as it was.
    let same = rec(s, "ws", "f", &batch("agent:a", "k", 5, 0));
    assert!(!same.changed);
    assert_eq!(same.header, a.header);
    let more = rec(s, "ws", "f", &batch("agent:a", "k", 5, 1));
    assert_eq!((more.changed, more.header.rev), (true, 2));
    let other = rec(s, "ws", "f", &batch("agent:a", "k2", 1, 1));
    assert_eq!(other.header.rev, 3, "a new header takes a rev too");
    let r = s
        .set_review("ws", "f", 1, Review::Accepted, "person:p", 5)
        .unwrap();
    assert_eq!((r.changed, r.header.rev), (true, 4));
    let r = s
        .set_review("ws", "f", 1, Review::Accepted, "person:q", 6)
        .unwrap();
    assert!(!r.changed, "the state it has");
    assert_eq!(r.header.rev, 4);
    assert_eq!(r.header.reviewed_by.as_deref(), Some("person:p"));
    // One counter per file: another file starts again.
    assert_eq!(
        rec(s, "ws", "g", &batch("agent:a", "k", 1, 1)).header.rev,
        1
    );
    assert_eq!(all(s, "ws", "f").iter().map(|h| h.rev).max(), Some(4));
}

pub fn an_update_never_lowers_and_may_raise_what_it_keeps(s: &dyn BatchStore) {
    let mut first = batch("agent:a", "k", 5, 2);
    first.first_seq = 10;
    first.last_seq = 11;
    first.at_ms = 7;
    first.atomic = false;
    first.tool = "import_tokens".into();
    let made = rec(s, "ws", "f", &first).header;
    assert_eq!((made.commands, made.written, made.last_seq), (5, 2, 11));
    // A later call with a lower count, nothing new and an older last seq changes nothing.
    let mut later = batch("agent:a", "k", 3, 0);
    (later.first_seq, later.last_seq, later.at_ms, later.atomic) = (1, 4, 99, true);
    later.tool = "apply_commands".into();
    let r = rec(s, "ws", "f", &later);
    assert!(!r.changed);
    assert_eq!(
        (r.header.commands, r.header.written, r.header.last_seq),
        (5, 2, 11)
    );
    // A retry that says more commands and wrote more raises them; the rest stays as the first call set it.
    let mut up = batch("agent:a", "k", 8, 3);
    (up.first_seq, up.last_seq, up.at_ms, up.atomic) = (2, 30, 500, true);
    let h = rec(s, "ws", "f", &up).header;
    assert_eq!((h.commands, h.written, h.last_seq), (8, 5, 30));
    assert_eq!((h.first_seq, h.at_ms, h.atomic), (10, 7, false));
    assert_eq!(h.tool, "import_tokens");
    assert_eq!(h.actor, "agent:a");
    // `written` stops at the bound.
    let mut big = batch("agent:a", "big", MAX_COUNT, MAX_COUNT);
    big.last_seq = 100;
    rec(s, "ws", "f", &big);
    let h = rec(s, "ws", "f", &batch("agent:a", "big", MAX_COUNT, 5)).header;
    assert_eq!(h.written, MAX_COUNT);
}

pub fn calls_that_know_what_the_log_holds_count_each_op_once(s: &dyn BatchStore) {
    let known = |prefix: &str, commands, written, k| {
        let mut b = batch("agent:a", prefix, commands, written);
        (b.known, b.last_seq) = (Some(k), 12);
        b
    };
    // A replay got there first and made the header at what the log holds; the delayed original, which wrote the same
    // three ops, adds nothing, and the other order ends the same.
    let h = rec(s, "ws", "f", &known("race", 3, 0, 3)).header;
    assert_eq!((h.commands, h.written), (3, 3));
    let r = rec(s, "ws", "f", &known("race", 3, 3, 3));
    assert!(!r.changed);
    assert_eq!((r.header.commands, r.header.written), (3, 3));
    rec(s, "ws", "f", &known("other", 3, 3, 3));
    let r = rec(s, "ws", "f", &known("other", 3, 0, 3));
    assert!(!r.changed);
    assert_eq!((r.header.commands, r.header.written), (3, 3));
    // A retry that stops again raises `written` to what the log holds, and a call that saw less never lowers it.
    assert_eq!(
        rec(s, "ws", "f", &known("again", 5, 0, 2)).header.written,
        2
    );
    let r = rec(s, "ws", "f", &known("again", 5, 0, 3));
    assert!(r.changed);
    assert_eq!(r.header.written, 3);
    let r = rec(s, "ws", "f", &known("again", 5, 0, 1));
    assert!(!r.changed);
    assert_eq!(r.header.written, 3);
    // What the log holds cannot pass the commands the batch has.
    let err = s.record("ws", "f", &known("bad", 2, 0, 3)).unwrap_err();
    assert!(matches!(err, ActivityError::Invalid(_)), "{err:?}");
    // With `known` given, `written` adds nothing, so a call that says it wrote more than the log holds is wrong.
    let err = s.record("ws", "f", &known("bad2", 5, 3, 2)).unwrap_err();
    assert!(matches!(err, ActivityError::Invalid(_)), "{err:?}");
}

pub fn commands_never_fall_below_written_across_the_parts_of_an_import(s: &dyn BatchStore) {
    // The parts of one import share a prefix and each says 100 commands, so the second makes 200 written.
    let one = rec(s, "ws", "f", &batch("agent:a", "imp", 100, 100)).header;
    assert_eq!((one.commands, one.written), (100, 100));
    let two = rec(s, "ws", "f", &batch("agent:a", "imp", 100, 100));
    assert!(two.changed);
    assert_eq!((two.header.commands, two.header.written), (200, 200));
    // A part that says the whole import has more commands raises them; a retry that wrote nothing keeps both.
    let three = rec(s, "ws", "f", &batch("agent:a", "imp", 250, 50)).header;
    assert_eq!((three.commands, three.written), (250, 250));
    let retry = rec(s, "ws", "f", &batch("agent:a", "imp", 300, 0)).header;
    assert_eq!((retry.commands, retry.written), (300, 250));
    // Whatever the order of the calls, a header never has more written than commands.
    for (i, (c, w)) in [(5, 5), (3, 3), (9, 1), (2, 2), (9, 0), (4, 4)]
        .into_iter()
        .enumerate()
    {
        let h = rec(s, "ws", "g", &batch("agent:a", "k", c, w)).header;
        assert!(h.written <= h.commands, "call {i}: {h:?}");
    }
}

pub fn the_first_note_wins_and_the_answer_says_which(s: &dyn BatchStore) {
    let r = rec(
        s,
        "ws",
        "f",
        &with_note(batch("agent:a", "k", 1, 1), "first"),
    );
    assert_eq!(r.note, NoteOutcome::Kept);
    assert_eq!(r.header.note.as_ref().map(Note::as_str), Some("first"));
    let r = rec(
        s,
        "ws",
        "f",
        &with_note(batch("agent:a", "k", 1, 0), "second"),
    );
    assert_eq!((r.note, r.changed), (NoteOutcome::KeptEarlier, false));
    assert_eq!(r.header.note.as_ref().map(Note::as_str), Some("first"));
    let r = rec(s, "ws", "f", &batch("agent:a", "k", 1, 0));
    assert_eq!((r.note, r.header.note.is_some()), (NoteOutcome::None, true));
    // A batch made with no note takes the first one a later call brings, and that is a change.
    let none = rec(s, "ws", "f", &batch("agent:a", "j", 2, 1));
    assert_eq!(none.note, NoteOutcome::None);
    let late = rec(
        s,
        "ws",
        "f",
        &with_note(batch("agent:a", "j", 2, 1), "late"),
    );
    assert_eq!((late.note, late.changed), (NoteOutcome::Kept, true));
    assert!(late.header.rev > none.header.rev);
    let multi = "line one\nline two \u{1F600}";
    let r = rec(s, "ws", "f", &with_note(batch("agent:a", "m", 1, 1), multi));
    assert_eq!(r.header.note.as_ref().map(Note::as_str), Some(multi));
    let listed = all(s, "ws", "f");
    assert_eq!(
        listed[0].note.as_ref().map(Note::as_str),
        Some(multi),
        "read back as written"
    );
}

pub fn a_review_is_set_by_a_person_and_survives_a_later_record(s: &dyn BatchStore) {
    rec(s, "ws", "f", &batch("agent:a", "k", 2, 1));
    let h = &all(s, "ws", "f")[0];
    assert_eq!(
        (h.review, h.reviewed_by.clone(), h.reviewed_at_ms),
        (Review::Unreviewed, None, None)
    );
    let r = s
        .set_review("ws", "f", 1, Review::NeedsReview, "person:p", 42)
        .unwrap();
    assert!(r.changed);
    assert_eq!(r.header.review, Review::NeedsReview);
    assert_eq!(r.header.reviewed_by.as_deref(), Some("person:p"));
    assert_eq!(r.header.reviewed_at_ms, Some(42));
    // The part that arrives later raises the count and leaves the person's mark alone.
    let up = rec(s, "ws", "f", &batch("agent:a", "k", 2, 1));
    assert!(up.changed);
    assert_eq!(up.header.written, 2);
    assert_eq!(up.header.review, Review::NeedsReview);
    assert_eq!(up.header.reviewed_by.as_deref(), Some("person:p"));
    // Clearing sets the state back and says who cleared it.
    let c = s
        .set_review("ws", "f", 1, Review::Unreviewed, "person:q", 50)
        .unwrap();
    assert_eq!(c.header.review, Review::Unreviewed);
    assert_eq!(
        (c.header.reviewed_by.as_deref(), c.header.reviewed_at_ms),
        (Some("person:q"), Some(50))
    );
}

pub fn a_review_of_a_missing_batch_or_one_of_another_workspace_is_not_found(s: &dyn BatchStore) {
    rec(s, "ws", "f", &batch("agent:a", "k", 1, 1));
    let missing = |r: Result<_, ActivityError>| {
        assert!(matches!(r, Err(ActivityError::NotFound(_))), "{r:?}")
    };
    missing(s.set_review("ws", "f", 2, Review::Accepted, "p", 1));
    missing(s.set_review("ws", "f", 0, Review::Accepted, "p", 1));
    missing(s.set_review("ws", "other", 1, Review::Accepted, "p", 1));
    missing(s.set_review("ws2", "f", 1, Review::Accepted, "p", 1));
    let bad = s.set_review("ws", "f", 1, Review::Accepted, "", 1);
    assert!(matches!(bad, Err(ActivityError::Invalid(_))), "{bad:?}");
    assert_eq!(
        all(s, "ws", "f")[0].review,
        Review::Unreviewed,
        "nothing changed"
    );
}

pub fn the_unreviewed_count_is_every_state_but_accepted(s: &dyn BatchStore) {
    let q = Query {
        before: None,
        after: None,
        limit: 10,
    };
    assert_eq!(s.list("ws", "f", &q).unwrap().unreviewed, 0);
    let mut last = 0;
    for p in ["a", "b", "c"] {
        last = rec(s, "ws", "f", &batch("agent:a", p, 1, 1)).unreviewed;
    }
    assert_eq!(last, 3);
    let r = |n, state| {
        s.set_review("ws", "f", n, state, "person:p", 9)
            .unwrap()
            .unreviewed
    };
    assert_eq!(r(1, Review::Accepted), 2);
    assert_eq!(r(2, Review::NeedsReview), 2, "flagged still counts");
    assert_eq!(r(3, Review::Accepted), 1);
    assert_eq!(r(2, Review::Accepted), 0);
    assert_eq!(r(2, Review::Accepted), 0, "the same state is a no-op");
    assert_eq!(s.list("ws", "f", &q).unwrap().unreviewed, 0);
    assert_eq!(r(1, Review::Unreviewed), 1);
    // A new batch and an update of an accepted one: the count moves only with headers that are not accepted.
    assert_eq!(
        rec(s, "ws", "f", &batch("agent:a", "d", 1, 1)).unreviewed,
        2
    );
    assert_eq!(
        rec(s, "ws", "f", &batch("agent:a", "b", 1, 0)).unreviewed,
        2
    );
    // It is the file's, whatever the page asks for, and another file has its own.
    let narrow = Query {
        before: Some(3),
        after: Some(1),
        limit: 1,
    };
    assert_eq!(s.list("ws", "f", &narrow).unwrap().unreviewed, 2);
    assert_eq!(
        rec(s, "ws", "g", &batch("agent:a", "a", 1, 1)).unreviewed,
        1
    );
}

pub fn a_list_is_newest_first_with_before_after_and_has_more(s: &dyn BatchStore) {
    let q = |before, after, limit| Query {
        before,
        after,
        limit,
    };
    let page = |b, a, l| {
        let p = s.list("ws", "f", &q(b, a, l)).unwrap();
        (numbers(&p.headers), p.has_more)
    };
    assert_eq!(
        page(None, None, 5),
        (vec![], false),
        "a file with none is an empty page"
    );
    for i in 0..7 {
        rec(s, "ws", "f", &batch("agent:a", &format!("k{i}"), 1, 1));
    }
    assert_eq!(page(None, None, 3), (vec![7, 6, 5], true));
    assert_eq!(page(Some(5), None, 3), (vec![4, 3, 2], true));
    assert_eq!(page(Some(2), None, 3), (vec![1], false));
    assert_eq!(page(None, None, 7), (vec![7, 6, 5, 4, 3, 2, 1], false));
    // Both bounds are exclusive and may be combined.
    assert_eq!(page(None, Some(4), 10), (vec![7, 6, 5], false));
    assert_eq!(page(None, Some(3), 2), (vec![7, 6], true));
    assert_eq!(page(Some(6), Some(2), 10), (vec![5, 4, 3], false));
    assert_eq!(page(Some(6), Some(2), 2), (vec![5, 4], true));
    assert_eq!(page(Some(1), None, 10), (vec![], false));
    assert_eq!(page(None, Some(7), 10), (vec![], false));
    for limit in [0, MAX_PAGE + 1] {
        let r = s.list("ws", "f", &q(None, None, limit));
        assert!(
            matches!(r, Err(ActivityError::Invalid(_))),
            "{limit}: {r:?}"
        );
    }
    assert!(s.list("ws", "f", &q(None, None, MAX_PAGE)).is_ok());
    // Another file, or another workspace, is an empty page and not an error.
    assert!(
        s.list("ws", "g", &q(None, None, 5))
            .unwrap()
            .headers
            .is_empty()
    );
    assert!(
        s.list("ws2", "f", &q(None, None, 5))
            .unwrap()
            .headers
            .is_empty()
    );
}

pub fn input_is_checked_before_anything_is_written(s: &dyn BatchStore) {
    let bad = |b: NewBatch| {
        let r = s.record("ws", "f", &b);
        assert!(matches!(r, Err(ActivityError::Invalid(_))), "{b:?}: {r:?}");
    };
    bad(batch("", "k", 1, 1));
    bad(batch(&"a".repeat(MAX_ACTOR + 1), "k", 1, 1));
    bad(batch("agent:\u{7}", "k", 1, 1));
    bad(batch("agent:\u{202E}x", "k", 1, 1));
    bad(batch("agent:a", "", 1, 1));
    bad(batch("agent:a", &"k".repeat(MAX_KEY_PREFIX + 1), 1, 1));
    bad(batch("agent:a", "k\n", 1, 1));
    bad(batch("agent:a", "k\u{200B}", 1, 1));
    bad(batch("agent:a", "k", 0, 0));
    bad(batch("agent:a", "k", MAX_COUNT + 1, 1));
    bad(batch("agent:a", "k", 2, 3));
    let mut b = batch("agent:a", "k", 1, 1);
    b.tool = String::new();
    bad(b.clone());
    b.tool = "t".repeat(MAX_TOOL + 1);
    bad(b);
    let mut b = batch("agent:a", "k", 1, 1);
    (b.first_seq, b.last_seq) = (5, 4);
    bad(b);
    // A sequence number or a time the database cannot hold is refused, not cut.
    let mut b = batch("agent:a", "k", 1, 1);
    b.last_seq = i64::MAX as u64 + 1;
    bad(b.clone());
    (b.last_seq, b.at_ms) = (5, u64::MAX);
    bad(b);
    // The longest names are fine, and a refusal took no number.
    let ok = batch(&"a".repeat(MAX_ACTOR), &"k".repeat(MAX_KEY_PREFIX), 1, 1);
    assert_eq!(rec(s, "ws", "f", &ok).header.n, 1);
}

pub fn the_caps_refuse_a_new_header_and_never_an_update(s: &dyn BatchStore) {
    let full = |r: Result<Recorded, ActivityError>, what: &str, limit: usize| match r {
        Err(ActivityError::Full { limit: l, what: w }) => assert_eq!((w, l), (what, limit)),
        other => panic!("expected {what}: {other:?}"),
    };
    // Per actor in a file: 3. The refused one took no number.
    for p in ["1", "2", "3"] {
        rec(s, "ws", "f", &batch("agent:a", p, 1, 1));
    }
    full(
        s.record("ws", "f", &batch("agent:a", "4", 1, 1)),
        ACTOR_HEADERS,
        3,
    );
    assert!(
        !rec(s, "ws", "f", &batch("agent:a", "2", 4, 1)).created,
        "an update is not refused"
    );
    assert_eq!(
        rec(s, "ws", "f", &batch("agent:b", "1", 1, 1)).header.n,
        4,
        "no hole"
    );
    // Another file has its own count for the actor; another workspace too.
    assert!(rec(s, "ws", "g", &batch("agent:a", "1", 1, 1)).created);
    assert!(rec(s, "ws2", "f", &batch("agent:a", "1", 1, 1)).created);
    // Per file: 5.
    rec(s, "ws", "f", &batch("agent:b", "2", 1, 1));
    full(
        s.record("ws", "f", &batch("agent:c", "1", 1, 1)),
        FILE_HEADERS,
        5,
    );
    // Per workspace: 8 (f has 5 and g has 1; two more fit).
    rec(s, "ws", "g", &batch("agent:a", "2", 1, 1));
    rec(s, "ws", "h", &batch("agent:d", "1", 1, 1));
    full(
        s.record("ws", "h", &batch("agent:d", "2", 1, 1)),
        WORKSPACE_HEADERS,
        8,
    );
    assert!(!rec(s, "ws", "h", &batch("agent:d", "1", 3, 0)).created);
    assert!(
        rec(s, "ws3", "h", &batch("agent:d", "2", 1, 1)).created,
        "another workspace"
    );
}
