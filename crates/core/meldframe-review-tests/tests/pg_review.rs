//! E5.4: the critique lists in Postgres. The store passes the cases of the in-memory one, a new process reads the
//! list back, and concurrent adds to one file never repeat a number.

#[path = "../../../../apps/server/tests/rooms_common/mod.rs"]
mod common;

use design_review::{ReviewError, ReviewStore, Status, contract};

macro_rules! case {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            let Some(db) = common::fresh() else {
                eprintln!("SKIPPED: DATABASE_URL is not set");
                return;
            };
            contract::$name(&db.store());
        }
    )*};
}

case!(
    numbers_start_at_one_and_never_repeat_per_file,
    files_and_workspaces_are_separate_lists,
    a_text_is_checked_before_anything_is_written,
    a_file_holds_a_limited_number_of_items_and_a_refused_add_writes_nothing,
    a_workspace_holds_a_limited_number_of_items_and_files,
    a_batch_is_linked_once_and_the_list_is_limited,
    an_item_is_marked_addressed_and_open_again,
    an_item_of_another_workspace_or_file_is_not_found,
);

#[test]
fn a_new_process_reads_the_list_back_as_it_was_left() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let s = db.store();
    s.add("ws", "f", "agent:a", &["one".into(), "two\nlines".into()])
        .unwrap();
    s.link("ws", "f", 2, "run1").unwrap();
    s.link("ws", "f", 2, "run2").unwrap();
    s.mark("ws", "f", 2, Status::Addressed).unwrap();
    let again = db.reconnect();
    let list = again.list("ws", "f").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        (list[1].text.as_str(), list[1].by.as_str(), list[1].status),
        ("two\nlines", "agent:a", Status::Addressed)
    );
    assert_eq!(list[1].batches, ["run1", "run2"]);
    assert_eq!(
        again.add("ws", "f", "u", &["three".into()]).unwrap()[0].n,
        3
    );
}

#[test]
fn adds_at_once_to_one_file_get_distinct_numbers_and_respect_the_limit() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = db.store();
    // All threads start together, so the adds race for the same next number.
    let start = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let (s, start) = (store.clone(), start.clone());
            std::thread::spawn(move || {
                start.wait();
                (0..30)
                    .map(|i| s.add("ws", "f", "a", &[format!("t{t} i{i}")]))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .flat_map(|t| t.join().unwrap())
        .collect();
    // Every try either wrote its item or was refused for the limit: a number clash (a store error) fails here.
    let mut written = 0;
    for r in &results {
        match r {
            Ok(_) => written += 1,
            Err(e) => assert_eq!(
                e,
                &ReviewError::Full {
                    limit: design_review::MAX_ITEMS,
                    what: design_review::FILE_ITEMS
                },
                "a refusal other than the limit"
            ),
        }
    }
    assert_eq!(results.len(), 240);
    assert_eq!(
        written,
        design_review::MAX_ITEMS,
        "240 tries, the limit wins"
    );
    let numbers: Vec<u32> = store.list("ws", "f").unwrap().iter().map(|i| i.n).collect();
    assert_eq!(numbers, (1..=written as u32).collect::<Vec<_>>());
}

#[test]
fn adds_at_once_to_many_files_respect_the_workspace_caps() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = db.store();
    let start = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let (s, start) = (store.clone(), start.clone());
            std::thread::spawn(move || {
                start.wait();
                (0..20)
                    .map(|i| s.add("ws", &format!("t{t}f{i}"), "a", &["x".into()]))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .flat_map(|t| t.join().unwrap())
        .collect();
    let written = results.iter().filter(|r| r.is_ok()).count();
    for r in results.iter().filter_map(|r| r.as_ref().err()) {
        assert!(matches!(r, ReviewError::Full { .. }), "{r:?}");
    }
    assert_eq!(
        written,
        design_review::MAX_WORKSPACE_FILES,
        "160 tries, 100 files"
    );
}

#[test]
fn links_at_once_at_the_batch_limit_keep_twenty_distinct_batches() {
    let Some(db) = common::fresh() else {
        eprintln!("SKIPPED: DATABASE_URL is not set");
        return;
    };
    let store = db.store();
    store.add("ws", "f", "a", &["one".into()]).unwrap();
    let start = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let (s, start) = (store.clone(), start.clone());
            std::thread::spawn(move || {
                start.wait();
                (0..5)
                    .map(|i| s.link("ws", "f", 1, &format!("b{t}-{i}")))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .flat_map(|t| t.join().unwrap())
        .collect();
    for r in results.iter().filter_map(|r| r.as_ref().err()) {
        assert!(matches!(r, ReviewError::Full { .. }), "{r:?}");
    }
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        design_review::MAX_BATCHES
    );
    let batches = &store.list("ws", "f").unwrap()[0].batches;
    let mut sorted = batches.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        (batches.len(), sorted.len()),
        (design_review::MAX_BATCHES, design_review::MAX_BATCHES)
    );
}
