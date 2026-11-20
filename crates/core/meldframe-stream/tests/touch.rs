//! Touch hints are coalesced per actor and file: the first one goes at once, later ones inside the window
//! replace each other (the latest node ids win) and go out when the window is over. The table is bounded.

use meldframe_stream::{
    Config, Frame, MAX_TOUCH_ENTRIES, MAX_TOUCH_NODES, Streams, TouchHint, Touches,
};

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

#[test]
fn a_clock_that_steps_back_does_not_silence_hints_or_pin_entries() {
    let mut t = Touches::new(100);
    assert!(t.record("f", "a", ids(&["n1"]), 1, 10_000).is_some());
    assert!(t.record("f", "a", ids(&["n2"]), 2, 10_050).is_none());
    // The clock steps back 5 s: the window counts as over, so the waiting hint goes out ...
    assert_eq!(t.due(5_000), vec![("f".to_owned(), hint("a", &["n2"], 2))]);
    // ... and a new one is not held back by a `last sent` that lies in the future.
    assert!(t.record("f", "a", ids(&["n3"]), 3, 4_990).is_some());

    let mut t = Touches::new(100);
    for i in 0..MAX_TOUCH_ENTRIES {
        assert!(
            t.record("f", &format!("actor{i}"), ids(&["n1"]), 1, 10_000)
                .is_some()
        );
    }
    // Entries that were sent "in the future" are idle, not inside their window: the table makes room.
    assert!(t.record("f", "late", ids(&["n1"]), 2, 5_000).is_some());
    assert!(t.len() <= MAX_TOUCH_ENTRIES);
}

fn stream_config(queue: usize) -> Config {
    Config {
        window: 8,
        queue,
        max_streams: 4,
        heartbeat_ms: 60_000,
        dead_after_ms: 120_000,
        stall_ms: 600_000,
    }
}

fn hints_streams(queue: usize) -> Streams<u64, String> {
    let mut s = Streams::new(stream_config(queue), 4);
    s.set_touch(0, |h| format!("touch:{}", h.actor));
    s
}

/// A touch hint may use only the queue minus a headroom (a 16th at least 1, at most 16 slots) that is kept
/// for durable frames and capture requests, so a hint never causes a resync or drops a capture request.
#[test]
fn touch_frames_leave_headroom_for_durable_frames_and_capture_requests() {
    let mut s = hints_streams(16);
    let id = s.subscribe("f", 0, 0, 0).unwrap().id;
    let queued: usize = (1..=40)
        .map(|seq| s.touch("f", "a", ids(&["n1"]), seq, 0))
        .sum();
    assert_eq!(
        queued, 14,
        "a queue of 16 keeps 2 slots free of touch frames"
    );
    assert_eq!(s.publish_ephemeral("f", None, "capture".into()), 1);
    s.publish("f", 1, 7).unwrap();
    let frames: Vec<_> = std::iter::from_fn(|| s.poll("f", id, 0)).collect();
    assert_eq!(frames.len(), 16);
    assert!(frames.contains(&Frame::Ephemeral("capture".into())));
    assert!(frames.contains(&Frame::Durable { seq: 1, data: 7 }));
    assert!(!frames.iter().any(|f| matches!(f, Frame::Resync { .. })));
}

/// The frame of a batch header uses the same headroom as a touch hint: it never takes the last slots of the queue and a
/// drop is silent (no resync). It does not cap the share of the rest.
#[test]
fn an_ephemeral_frame_with_headroom_leaves_the_slots_durable_frames_need() {
    let mut s = hints_streams(16);
    let id = s.subscribe("f", 0, 0, 0).unwrap().id;
    let queued: usize = (0..40)
        .map(|i| s.publish_ephemeral_with_headroom("f", None, format!("batch{i}")))
        .sum();
    assert_eq!(queued, 14, "a queue of 16 keeps 2 slots free");
    assert_eq!(
        s.publish_ephemeral_with_headroom("nokey", None, "x".into()),
        0
    );
    s.publish("f", 1, 7).unwrap();
    s.publish("f", 2, 8).unwrap();
    let frames: Vec<_> = std::iter::from_fn(|| s.poll("f", id, 0)).collect();
    assert_eq!(frames.len(), 16);
    assert!(frames.contains(&Frame::Durable { seq: 2, data: 8 }));
    assert!(!frames.iter().any(|f| matches!(f, Frame::Resync { .. })));
}

#[test]
fn with_room_in_the_queue_a_touch_is_delivered() {
    let mut s = hints_streams(16);
    let id = s.subscribe("f", 0, 0, 0).unwrap().id;
    assert_eq!(s.touch("f", "a", ids(&["n1"]), 1, 0), 1);
    assert_eq!(
        s.poll("f", id, 0),
        Some(Frame::Ephemeral("touch:a".to_owned()))
    );
}
