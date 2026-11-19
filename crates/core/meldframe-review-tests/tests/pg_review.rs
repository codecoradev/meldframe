//! E5.4: the critique lists in Postgres. The store passes the cases of the in-memory one, a new process reads the
//! list back, and concurrent adds to one file never repeat a number.

#[path = "../../meldframe-server-tests/tests/common/mod.rs"]
mod common;

use design_review::{ReviewStore, Status, contract};

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
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let s = store.clone();
            std::thread::spawn(move || {
                (0..30)
                    .filter_map(|i| s.add("ws", "f", "a", &[format!("t{t} i{i}")]).ok())
                    .count()
            })
        })
        .collect();
    let written: usize = threads.into_iter().map(|t| t.join().unwrap()).sum();
    let list = store.list("ws", "f").unwrap();
    assert_eq!(
        written,
        design_review::MAX_ITEMS,
        "240 tries, the limit wins"
    );
    let numbers: Vec<u32> = list.iter().map(|i| i.n).collect();
    assert_eq!(numbers, (1..=written as u32).collect::<Vec<_>>());
}
