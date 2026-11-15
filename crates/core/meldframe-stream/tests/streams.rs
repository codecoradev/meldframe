//! Many hubs under one server-wide cap.

use meldframe_stream::{Config, Frame, JoinError, StreamError, Streams};

fn cfg() -> Config {
    Config {
        window: 4,
        queue: 3,
        max_streams: 2,
        heartbeat_ms: 100,
        dead_after_ms: 350,
        stall_ms: 200,
    }
}

type S = Streams<u64, ()>;

#[test]
fn publishes_reach_the_streams_of_their_key_only() {
    let mut s = S::new(cfg(), 8);
    let a = s.subscribe("f", 0, 0, 0).unwrap().id;
    let b = s.subscribe("g", 0, 0, 0).unwrap().id;
    s.publish("f", 1, 10).unwrap();
    s.publish("none", 1, 10).unwrap(); // no hub: nothing to do
    assert_eq!(s.poll("f", a, 0), Some(Frame::Durable { seq: 1, data: 10 }));
    assert_eq!(s.poll("g", b, 0), None);
    assert_eq!(s.publish_ephemeral("g", None, ()), 1);
    assert_eq!(s.poll("g", b, 0), Some(Frame::Ephemeral(())));
}

#[test]
fn the_total_cap_spans_keys_and_a_closed_stream_frees_a_slot() {
    let mut s = S::new(cfg(), 2);
    let a = s.subscribe("f", 0, 0, 0).unwrap().id;
    s.subscribe("g", 0, 0, 0).unwrap();
    assert_eq!(s.subscribe("h", 0, 0, 0).err(), Some(StreamError::Full));
    assert_eq!(s.stream_count(), 2);
    s.unsubscribe("f", a);
    s.subscribe("h", 0, 0, 0).unwrap();
}

#[test]
fn a_hub_ends_with_its_last_stream_and_the_next_starts_at_the_given_latest() {
    let mut s = S::new(cfg(), 8);
    let a = s.subscribe("f", 0, 0, 0).unwrap().id;
    s.publish("f", 1, 10).unwrap();
    s.unsubscribe("f", a);
    let again = s.subscribe("f", 5, 1, 0).unwrap();
    assert_eq!(
        again.missing,
        Some((1, 5)),
        "empty window: all from the store"
    );
}

#[test]
fn tick_names_the_silent_streams_and_resubscribe_needs_a_known_stream() {
    let mut s = S::new(cfg(), 8);
    let a = s.subscribe("f", 0, 0, 0).unwrap().id;
    assert_eq!(s.tick(350), vec![("f".to_string(), a)]);
    assert_eq!(s.stream_count(), 0);
    assert_eq!(
        s.resubscribe("f", a, 0, 400).err(),
        Some(StreamError::Unknown)
    );
}

#[test]
fn join_reads_the_range_older_than_the_window_and_reports_a_failed_read() {
    let mut s = S::new(cfg(), 8);
    let got = s.join("f", 3, 1, 0, 100, |a, b| Ok::<_, ()>((a + 1..=b).collect()));
    assert_eq!(got.unwrap().replay, vec![2, 3]);
    let a = s.join("g", 3, 3, 0, 100, |_, _| Err::<Vec<u64>, _>("never called"));
    assert!(a.unwrap().replay.is_empty(), "the window covers it");
    let bad = s.join("h", 3, 0, 0, 100, |_, _| Err::<Vec<u64>, _>("down"));
    assert_eq!(bad.err(), Some(JoinError::Read("down")));
    assert_eq!(s.stream_count(), 2, "a failed read leaves no stream behind");
    let id = s
        .join("g", 3, 0, 0, 100, |_, _| Ok::<_, ()>(vec![1]))
        .unwrap()
        .id;
    assert_eq!(
        s.rejoin("g", id, 0, 0, 100, |_, _| Ok::<_, ()>(vec![9]))
            .unwrap()
            .replay,
        vec![9]
    );
}

#[test]
fn a_failed_rejoin_keeps_the_old_stream_and_its_queue() {
    let mut s = S::new(cfg(), 8);
    let id = s.subscribe("f", 5, 5, 0).unwrap().id;
    s.publish("f", 6, 60).unwrap();
    let bad = s.rejoin("f", id, 0, 0, 100, |_, _| Err::<Vec<u64>, _>("down"));
    assert_eq!(bad.err(), Some(JoinError::Read("down")));
    assert_eq!(s.stream_count(), 1);
    assert_eq!(
        s.poll("f", id, 0),
        Some(Frame::Durable { seq: 6, data: 60 })
    );
}

#[test]
fn a_stream_that_is_seen_survives_a_tick_and_an_unseen_one_is_dropped_and_frees_its_slot() {
    let mut s = S::new(cfg(), 1);
    let a = s.subscribe("f", 0, 0, 0).unwrap().id;
    s.seen("f", a, 300);
    assert!(s.tick(350).is_empty());
    assert_eq!(s.tick(650), vec![("f".to_string(), a)]);
    assert_eq!(s.stream_count(), 0);
    s.subscribe("f", 0, 0, 650).unwrap();
}

#[test]
fn a_stream_stalled_longer_than_the_bound_is_reaped_even_when_it_pings() {
    let mut s = S::new(cfg(), 8);
    let a = s.subscribe("f", 0, 0, 0).unwrap().id;
    for seq in 1..=4 {
        s.publish("f", seq, 0).unwrap(); // queue of 3 overflows: stalled
    }
    s.seen("f", a, 100);
    assert!(s.tick(100).is_empty(), "the stall is first seen at 100");
    s.seen("f", a, 250);
    assert!(s.tick(299).is_empty());
    s.seen("f", a, 300);
    assert_eq!(s.tick(300), vec![("f".to_string(), a)]);
}

#[test]
fn a_stream_id_is_never_reused_after_its_hub_is_recreated() {
    let mut s = S::new(cfg(), 8);
    let old = s.subscribe("f", 0, 0, 0).unwrap().id;
    s.unsubscribe("f", old);
    let new = s.subscribe("f", 0, 0, 0).unwrap().id;
    assert_ne!(old, new);
    s.publish("f", 1, 10).unwrap();
    assert_eq!(s.poll("f", old, 0), None);
    assert_eq!(s.publish_ephemeral("f", Some(old), ()), 0);
    assert_eq!(
        s.poll("f", new, 0),
        Some(Frame::Durable { seq: 1, data: 10 })
    );
}

#[test]
fn a_long_replay_is_cut_and_the_rest_comes_by_resubscribing_from_the_marker() {
    let mut s = S::new(cfg(), 8);
    let read = |a: u64, b: u64| Ok::<_, ()>((a + 1..=b).collect::<Vec<u64>>());
    let j = s.join("f", 5, 0, 0, 2, read).unwrap();
    assert_eq!(j.replay, vec![1, 2]);
    assert_eq!(
        s.poll("f", j.id, 0),
        Some(Frame::Resync {
            after_seq: 2,
            latest_seq: 5
        })
    );
    assert_eq!(
        s.rejoin("f", j.id, 2, 0, 2, read).unwrap().replay,
        vec![3, 4]
    );
    assert_eq!(
        s.poll("f", j.id, 0),
        Some(Frame::Resync {
            after_seq: 4,
            latest_seq: 5
        })
    );
    assert_eq!(s.rejoin("f", j.id, 4, 0, 2, read).unwrap().replay, vec![5]);
    assert_eq!(s.poll("f", j.id, 0), None, "caught up: live frames follow");
    s.publish("f", 6, 60).unwrap();
    assert_eq!(
        s.poll("f", j.id, 0),
        Some(Frame::Durable { seq: 6, data: 60 })
    );
}
