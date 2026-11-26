//! The cases every [`ReviewStore`] passes: the in-memory store and the Postgres one run the same functions, each on
//! an empty store. A case panics when the store breaks a rule.

use crate::{
    FILE_ITEMS, ITEM_BATCHES, Item, MAX_BATCHES, MAX_ITEMS, MAX_TEXT, MAX_WORKSPACE_FILES,
    MAX_WORKSPACE_ITEMS, ReviewError, ReviewStore, Status, WORKSPACE_FILES, WORKSPACE_ITEMS,
    check_batch,
};

fn texts(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("item {i}")).collect()
}

fn numbers(items: &[Item]) -> Vec<u32> {
    items.iter().map(|i| i.n).collect()
}

pub fn numbers_start_at_one_and_never_repeat_per_file(s: &dyn ReviewStore) {
    assert_eq!(s.list("ws", "f").unwrap(), Vec::new(), "a file with none");
    let first = s
        .add(
            "ws",
            "f",
            "agent:a",
            &["hero is cramped".into(), " low contrast ".into()],
        )
        .unwrap();
    assert_eq!(numbers(&first), [1, 2]);
    assert_eq!(first[1].text, "low contrast", "trimmed");
    assert_eq!(
        (first[0].by.as_str(), first[0].status),
        ("agent:a", Status::Open)
    );
    let second = s
        .add("ws", "f", "user:b", &["button is small".into()])
        .unwrap();
    assert_eq!(numbers(&second), [3]);
    assert_eq!(second[0].by, "user:b");
    let all = s.list("ws", "f").unwrap();
    assert_eq!(numbers(&all), [1, 2, 3]);
    assert_eq!(all[2], second[0]);
}

pub fn files_and_workspaces_are_separate_lists(s: &dyn ReviewStore) {
    s.add("ws_a", "f1", "agent:a", &["one".into(), "two".into()])
        .unwrap();
    // Another file, and another workspace with the same file name, start at 1 and see nothing of it.
    assert_eq!(
        numbers(&s.add("ws_a", "f2", "agent:a", &["x".into()]).unwrap()),
        [1]
    );
    assert_eq!(
        numbers(&s.add("ws_b", "f1", "agent:z", &["y".into()]).unwrap()),
        [1]
    );
    assert_eq!(s.list("ws_b", "f1").unwrap().len(), 1);
    assert_eq!(s.list("ws_a", "f1").unwrap().len(), 2);
    assert_eq!(s.list("ws_b", "f2").unwrap(), Vec::new());
    // The key is not the two names glued together: ws "a/b" file "c" is not ws "a" file "b/c".
    s.add("a/b", "c", "agent:a", &["p".into()]).unwrap();
    assert_eq!(s.list("a", "b/c").unwrap(), Vec::new());
}

pub fn a_text_is_checked_before_anything_is_written(s: &dyn ReviewStore) {
    let invalid = |r: Result<Vec<Item>, ReviewError>| {
        assert!(matches!(r, Err(ReviewError::Invalid(_))), "{r:?}")
    };
    invalid(s.add("ws", "f", "a", &[]));
    invalid(s.add("ws", "f", "a", &texts(21)));
    invalid(s.add("ws", "f", "a", &["fine".into(), "   ".into()]));
    invalid(s.add("ws", "f", "a", &["x".repeat(MAX_TEXT + 1)]));
    invalid(s.add("ws", "f", "a", &["bell\u{7}".into()]));
    invalid(s.add("ws", "f", "a", &["a\tb".into()]));
    invalid(s.add("ws", "f", "a", &["a\r\nb".into()]));
    // Characters that change how text reads without showing: bidi override, zero width, separators, BOM.
    for c in [
        '\u{202E}', '\u{200B}', '\u{200D}', '\u{2028}', '\u{2029}', '\u{FEFF}', '\u{2066}',
    ] {
        invalid(s.add("ws", "f", "a", &[format!("pay{c}ment")]));
    }
    // The ones the old set of this check did not hold and the one set of `meldframe-text` does (U9): the combining
    // grapheme joiner, the Hangul filler, the variation selector, a tag character, the soft hyphen.
    for c in [
        '\u{034F}',
        '\u{3164}',
        '\u{FE0F}',
        '\u{E0041}',
        '\u{00AD}',
        '\u{2060}',
    ] {
        invalid(s.add("ws", "f", "a", &[format!("pay{c}ment")]));
    }
    // The limit counts characters, not bytes.
    invalid(s.add("ws", "f", "a", &["\u{e9}".repeat(MAX_TEXT + 1)]));
    assert_eq!(
        s.list("ws", "f").unwrap(),
        Vec::new(),
        "nothing was written"
    );
    // The limits themselves fit; a line break is plain text.
    s.add("ws", "f", "a", &["x".repeat(MAX_TEXT), "two\nlines".into()])
        .unwrap();
    s.add("ws", "g", "a", &texts(20)).unwrap();
    s.add("ws", "h", "a", &["\u{e9}".repeat(MAX_TEXT)]).unwrap();
}

pub fn a_workspace_holds_a_limited_number_of_items_and_files(s: &dyn ReviewStore) {
    // Files that do not exist in the design count too: a name is enough to have a list.
    let per_file = MAX_ITEMS;
    for i in 0..MAX_WORKSPACE_ITEMS / per_file {
        for _ in 0..per_file / 20 {
            s.add("ws", &format!("f{i}"), "a", &texts(20)).unwrap();
        }
    }
    let over = Err(ReviewError::Full {
        limit: MAX_WORKSPACE_ITEMS,
        what: WORKSPACE_ITEMS,
    });
    assert_eq!(s.add("ws", "fresh", "a", &texts(1)), over);
    assert_eq!(
        s.list("ws", "fresh").unwrap(),
        Vec::new(),
        "nothing was written"
    );
    // Another workspace has its own room.
    s.add("ws2", "f0", "a", &texts(20)).unwrap();
    // Files: one item each, up to the cap; an existing file still takes items, a new one does not.
    for i in 0..MAX_WORKSPACE_FILES {
        s.add("wf", &format!("f{i}"), "a", &texts(1)).unwrap();
    }
    assert_eq!(
        s.add("wf", "one more", "a", &texts(1)),
        Err(ReviewError::Full {
            limit: MAX_WORKSPACE_FILES,
            what: WORKSPACE_FILES
        })
    );
    assert_eq!(s.list("wf", "one more").unwrap(), Vec::new());
    assert_eq!(numbers(&s.add("wf", "f0", "a", &texts(1)).unwrap()), [2]);
}

pub fn a_file_holds_a_limited_number_of_items_and_a_refused_add_writes_nothing(
    s: &dyn ReviewStore,
) {
    for _ in 0..MAX_ITEMS / 20 - 1 {
        s.add("ws", "f", "a", &texts(20)).unwrap();
    }
    s.add("ws", "f", "a", &texts(19)).unwrap();
    assert_eq!(s.list("ws", "f").unwrap().len(), MAX_ITEMS - 1);
    // Two do not fit; none is written, and the one that fits still does.
    assert_eq!(
        s.add("ws", "f", "a", &texts(2)),
        Err(ReviewError::Full {
            limit: MAX_ITEMS,
            what: FILE_ITEMS
        })
    );
    assert_eq!(s.list("ws", "f").unwrap().len(), MAX_ITEMS - 1);
    let last = s.add("ws", "f", "a", &texts(1)).unwrap();
    assert_eq!(numbers(&last), [MAX_ITEMS as u32]);
    assert!(matches!(
        s.add("ws", "f", "a", &texts(1)),
        Err(ReviewError::Full { .. })
    ));
    // Another file of the workspace has its own room.
    s.add("ws", "other", "a", &texts(1)).unwrap();
}

pub fn a_batch_is_linked_once_and_the_list_is_limited(s: &dyn ReviewStore) {
    s.add("ws", "f", "a", &texts(2)).unwrap();
    assert_eq!(s.link("ws", "f", 1, "run1").unwrap().batches, ["run1"]);
    assert_eq!(
        s.link("ws", "f", 1, "run2").unwrap().batches,
        ["run1", "run2"]
    );
    assert_eq!(
        s.link("ws", "f", 1, "run1").unwrap().batches,
        ["run1", "run2"],
        "again: no change"
    );
    assert_eq!(
        s.list("ws", "f").unwrap()[1].batches,
        Vec::<String>::new(),
        "item 2 has none"
    );
    assert!(matches!(
        s.link("ws", "f", 1, ""),
        Err(ReviewError::Invalid(_))
    ));
    assert!(check_batch(&"k".repeat(101)).is_err());
    // A key is counted in characters (a key of 100 four-byte characters is taken) and holds no hidden character.
    assert!(check_batch(&"\u{1F600}".repeat(100)).is_ok());
    for c in ['\u{202E}', '\u{200B}', '\u{2028}', '\u{034F}', '\u{200D}'] {
        assert!(
            check_batch(&format!("run{c}1")).is_err(),
            "U+{:04X}",
            c as u32
        );
    }
    for i in 2..MAX_BATCHES {
        s.link("ws", "f", 1, &format!("run{}", i + 1)).unwrap();
    }
    assert_eq!(s.list("ws", "f").unwrap()[0].batches.len(), MAX_BATCHES);
    assert_eq!(
        s.link("ws", "f", 1, "one too many"),
        Err(ReviewError::Full {
            limit: MAX_BATCHES,
            what: ITEM_BATCHES
        })
    );
    // A batch already linked is not "one more", even at the limit.
    assert!(s.link("ws", "f", 1, "run1").is_ok());
}

pub fn an_item_is_marked_addressed_and_open_again(s: &dyn ReviewStore) {
    s.add("ws", "f", "a", &texts(2)).unwrap();
    assert_eq!(
        s.mark("ws", "f", 2, Status::Addressed).unwrap().status,
        Status::Addressed
    );
    assert_eq!(
        s.mark("ws", "f", 2, Status::Addressed).unwrap().status,
        Status::Addressed
    );
    let all = s.list("ws", "f").unwrap();
    assert_eq!(
        (all[0].status, all[1].status),
        (Status::Open, Status::Addressed)
    );
    assert_eq!(
        s.mark("ws", "f", 2, Status::Open).unwrap().status,
        Status::Open
    );
    // Marking keeps the links.
    s.link("ws", "f", 1, "run1").unwrap();
    assert_eq!(
        s.mark("ws", "f", 1, Status::Addressed).unwrap().batches,
        ["run1"]
    );
}

pub fn an_item_of_another_workspace_or_file_is_not_found(s: &dyn ReviewStore) {
    s.add("ws_a", "f", "a", &texts(1)).unwrap();
    let gone = Err(ReviewError::ItemNotFound(1));
    assert_eq!(s.link("ws_b", "f", 1, "run1"), gone);
    assert_eq!(s.mark("ws_b", "f", 1, Status::Addressed), gone);
    assert_eq!(s.link("ws_a", "other", 1, "run1"), gone);
    assert_eq!(s.mark("ws_a", "other", 1, Status::Addressed), gone);
    assert_eq!(
        s.link("ws_a", "f", 2, "run1"),
        Err(ReviewError::ItemNotFound(2))
    );
    // Item numbers start at 1: 0 is refused as input, before the batch is looked at.
    for r in [
        s.link("ws_a", "f", 0, "run1"),
        s.link("ws_a", "f", 0, ""),
        s.mark("ws_a", "f", 0, Status::Open),
    ] {
        assert!(matches!(r, Err(ReviewError::Invalid(_))), "{r:?}");
    }
    // Nothing of the failed calls reached ws_a's item.
    let item = &s.list("ws_a", "f").unwrap()[0];
    assert_eq!((item.status, item.batches.len()), (Status::Open, 0));
}
