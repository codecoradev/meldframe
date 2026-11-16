//! Touch hints are coalesced per actor and file: the first one goes at once, later ones inside the window
//! replace each other (the latest node ids win) and go out when the window is over. The table is bounded.

use meldframe_stream::{MAX_TOUCH_ENTRIES, MAX_TOUCH_NODES, TouchHint, Touches};

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

fn hint(actor: &str, nodes: &[&str], seq: u64) -> TouchHint {
    TouchHint {
        actor: actor.into(),
        node_ids: ids(nodes),
        seq,
    }
}

#[test]
fn the_first_touch_goes_at_once_and_a_later_one_in_the_window_waits() {
    let mut t = Touches::new(100);
    assert_eq!(
        t.record("f", "a", ids(&["n1"]), 1, 1000),
        Some(hint("a", &["n1"], 1))
    );
    assert_eq!(t.record("f", "a", ids(&["n2"]), 2, 1099), None);
    assert_eq!(t.record("f", "a", ids(&["n3"]), 3, 1099), None);
    assert!(
        t.due(1099).is_empty(),
        "99 ms after the last frame is too early"
    );
    // The latest node ids win; the frame goes out when 100 ms have passed since the last one.
    assert_eq!(t.due(1100), vec![("f".to_owned(), hint("a", &["n3"], 3))]);
    assert!(t.due(1500).is_empty(), "a flushed hint is not sent twice");
}

#[test]
fn a_touch_after_the_window_goes_at_once_and_drops_a_pending_one() {
    let mut t = Touches::new(100);
    assert!(t.record("f", "a", ids(&["n1"]), 1, 0).is_some());
    assert_eq!(t.record("f", "a", ids(&["n2"]), 2, 50), None);
    // Nobody called `due`: the newer touch supersedes the pending one instead of sending both.
    assert_eq!(
        t.record("f", "a", ids(&["n3"]), 3, 400),
        Some(hint("a", &["n3"], 3))
    );
    assert!(t.due(1000).is_empty());
}

#[test]
fn each_actor_and_each_file_has_its_own_window() {
    let mut t = Touches::new(100);
    assert!(t.record("f", "a", ids(&["n1"]), 1, 0).is_some());
    assert!(
        t.record("f", "b", ids(&["n1"]), 2, 1).is_some(),
        "another actor"
    );
    assert!(
        t.record("g", "a", ids(&["n1"]), 3, 2).is_some(),
        "another file"
    );
    assert!(t.record("f", "a", ids(&["n2"]), 4, 3).is_none());
}

#[test]
fn a_hint_names_each_node_once_and_at_most_the_cap() {
    let mut t = Touches::new(100);
    let many: Vec<String> = (0..(MAX_TOUCH_NODES + 40))
        .map(|i| format!("n{}", i % (MAX_TOUCH_NODES + 20)))
        .collect();
    let sent = t.record("f", "a", many, 1, 0).unwrap();
    assert_eq!(sent.node_ids.len(), MAX_TOUCH_NODES);
    let mut sorted = sent.node_ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), MAX_TOUCH_NODES);
    assert!(
        t.record("f", "b", Vec::new(), 2, 0).is_none(),
        "no node, no hint"
    );
}

#[test]
fn the_table_is_bounded_and_idle_entries_make_room() {
    let mut t = Touches::new(100);
    for i in 0..(MAX_TOUCH_ENTRIES + 500) {
        let _ = t.record("f", &format!("actor{i}"), ids(&["n1"]), 1, 0);
    }
    assert!(t.len() <= MAX_TOUCH_ENTRIES);
    // The table is full of entries inside their window: a new actor gets no hint (best effort), nothing grows.
    assert_eq!(t.record("f", "late", ids(&["n1"]), 2, 10), None);
    assert!(t.len() <= MAX_TOUCH_ENTRIES);
    // Once the windows are over the idle entries are dropped and a new actor is served again.
    assert!(t.record("f", "later", ids(&["n1"]), 3, 500).is_some());
    assert!(t.len() <= MAX_TOUCH_ENTRIES);
}

#[test]
fn a_window_of_zero_sends_every_touch() {
    let mut t = Touches::new(0);
    for seq in 1..=5 {
        assert!(t.record("f", "a", ids(&["n1"]), seq, 7).is_some());
    }
    assert!(t.due(7).is_empty());
}
