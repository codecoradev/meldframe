//! The stream seam (ADR-0009): durable frames with sequence numbers, replay from a cursor, ephemeral frames,
//! bounded queues with a resync marker, the stream cap and the heartbeat schedule. No transport, no clock
//! of its own: every call that needs time takes it as an argument.

use meldframe_stream::{Config, Frame, Hub, StreamError, StreamId};

type H = Hub<String, String>;
type F = Frame<String, String>;

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

fn drain(h: &mut H, id: StreamId, now: u64) -> Vec<F> {
    std::iter::from_fn(|| h.poll(id, now)).collect()
}

fn seqs(frames: &[F]) -> Vec<u64> {
    frames
        .iter()
        .filter_map(|f| match f {
            Frame::Durable { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect()
}

fn publish(h: &mut H, from: u64, to: u64) {
    for s in from..=to {
        h.publish(s, format!("op{s}")).unwrap();
    }
}

#[test]
fn every_subscriber_gets_durable_frames_in_order_with_their_numbers() {
    let mut h = H::new(cfg(), 0);
    let a = h.subscribe(0, 0).unwrap().id;
    let b = h.subscribe(0, 0).unwrap().id;
    publish(&mut h, 1, 3);
    for id in [a, b] {
        let got = drain(&mut h, id, 0);
        assert_eq!(seqs(&got), vec![1, 2, 3]);
        assert!(matches!(&got[1], Frame::Durable { data, .. } if data == "op2"));
    }
}

#[test]
fn reconnect_from_a_number_in_the_window_gets_exactly_the_missed_frames() {
    let mut h = H::new(cfg(), 0);
    publish(&mut h, 1, 4);
    let s = h.subscribe(2, 0).unwrap();
    assert_eq!(s.missing, None);
    assert_eq!(seqs(&drain(&mut h, s.id, 0)), vec![3, 4]);
    publish(&mut h, 5, 5);
    assert_eq!(seqs(&drain(&mut h, s.id, 0)), vec![5]);
}

#[test]
fn reconnect_at_the_latest_number_gets_nothing_until_the_next_op() {
    let mut h = H::new(cfg(), 0);
    publish(&mut h, 1, 2);
    let s = h.subscribe(2, 0).unwrap();
    assert!(drain(&mut h, s.id, 0).is_empty());
}

#[test]
fn a_client_behind_the_window_is_told_which_range_to_read_from_the_store() {
    let mut h = H::new(Config { queue: 5, ..cfg() }, 0);
    publish(&mut h, 1, 6); // window keeps 3..=6
    let s = h.subscribe(1, 0).unwrap();
    assert_eq!(s.missing, Some((1, 2)), "2 and then the window from 3");
    assert_eq!(seqs(&drain(&mut h, s.id, 0)), vec![3, 4, 5, 6]);
}

#[test]
fn a_hub_started_late_has_an_empty_window_and_sends_all_history_from_the_store() {
    let mut h = H::new(cfg(), 10);
    let s = h.subscribe(4, 0).unwrap();
    assert_eq!(s.missing, Some((4, 10)));
    publish(&mut h, 11, 11);
    assert_eq!(seqs(&drain(&mut h, s.id, 0)), vec![11]);
}

#[test]
fn a_client_ahead_of_the_log_gets_a_resync_marker() {
    let mut h = H::new(cfg(), 5);
    let s = h.subscribe(9, 0).unwrap();
    assert_eq!(
        drain(&mut h, s.id, 0),
        vec![Frame::Resync {
            after_seq: 5,
            latest_seq: 5
        }]
    );
}

#[test]
fn publish_refuses_a_gap_and_ignores_a_repeat() {
    let mut h = H::new(cfg(), 0);
    publish(&mut h, 1, 2);
    assert_eq!(
        h.publish(5, "x".into()),
        Err(StreamError::Gap {
            expected: 3,
            got: 5
        })
    );
    // The hub resyncs past the gap: it has no window across it, the store covers the range.
    assert_eq!(h.latest(), 5);
    h.publish(2, "again".into()).unwrap();
    let s = h.subscribe(0, 0).unwrap();
    assert_eq!(s.missing, Some((0, 5)));
}

#[test]
fn ephemeral_frames_have_no_number_and_are_never_replayed() {
    let mut h = H::new(cfg(), 0);
    let a = h.subscribe(0, 0).unwrap().id;
    let b = h.subscribe(0, 0).unwrap().id;
    assert_eq!(h.publish_ephemeral(None, "touch".into()), 2);
    assert_eq!(h.publish_ephemeral(Some(a), "capture".into()), 1);
    assert_eq!(
        drain(&mut h, a, 0),
        vec![
            Frame::Ephemeral("touch".to_string()),
            Frame::Ephemeral("capture".to_string())
        ]
    );
    assert_eq!(drain(&mut h, b, 0), vec![Frame::Ephemeral("touch".into())]);
    h.unsubscribe(a);
    let late = h.subscribe(0, 0).unwrap();
    assert!(drain(&mut h, late.id, 0).is_empty());
}

#[test]
fn an_ephemeral_frame_to_a_gone_stream_reaches_nobody() {
    let mut h = H::new(cfg(), 0);
    let a = h.subscribe(0, 0).unwrap().id;
    h.unsubscribe(a);
    assert_eq!(h.publish_ephemeral(Some(a), "capture".into()), 0);
}

#[test]
fn a_stalled_client_gets_a_resync_marker_and_the_publisher_never_waits() {
    let mut h = H::new(cfg(), 0);
    let stalled = h.subscribe(0, 0).unwrap().id;
    let live = h.subscribe(0, 0).unwrap().id;
    publish(&mut h, 1, 3);
    assert_eq!(seqs(&drain(&mut h, live, 0)), vec![1, 2, 3]);
    for s in 4..=8 {
        // the stalled queue (3) is full since frame 3; every publish still returns at once
        publish(&mut h, s, s);
        assert_eq!(seqs(&drain(&mut h, live, 0)), vec![s]);
    }
    let got = drain(&mut h, stalled, 0);
    assert_eq!(seqs(&got), vec![1, 2, 3]);
    assert_eq!(
        got.last(),
        Some(&Frame::Resync {
            after_seq: 3,
            latest_seq: 8
        })
    );
    // Frames after the marker are not queued for it.
    publish(&mut h, 9, 9);
    assert!(drain(&mut h, stalled, 0).is_empty());
}

#[test]
fn a_resynced_client_recovers_from_the_number_in_the_marker() {
    let mut h = H::new(
        Config {
            window: 10,
            ..cfg()
        },
        0,
    );
    let id = h.subscribe(0, 0).unwrap().id;
    publish(&mut h, 1, 6);
    let got = drain(&mut h, id, 0);
    let Some(Frame::Resync { after_seq, .. }) = got.last().cloned() else {
        panic!("no marker: {got:?}");
    };
    let again = h.resubscribe(id, after_seq, 0).unwrap();
    assert_eq!(again.id, id);
    assert_eq!(seqs(&drain(&mut h, id, 0)), vec![4, 5, 6]);
    publish(&mut h, 7, 7);
    assert_eq!(seqs(&drain(&mut h, id, 0)), vec![7]);
}

#[test]
fn a_full_queue_drops_ephemeral_frames_without_a_marker() {
    let mut h = H::new(cfg(), 0);
    let id = h.subscribe(0, 0).unwrap().id;
    for i in 0..5 {
        h.publish_ephemeral(Some(id), format!("t{i}"));
    }
    let got = drain(&mut h, id, 0);
    assert_eq!(got.len(), 3);
    assert!(got.iter().all(|f| matches!(f, Frame::Ephemeral(_))));
}

#[test]
fn streams_are_capped_and_a_closed_stream_frees_its_slot() {
    let mut h = H::new(cfg(), 0);
    let a = h.subscribe(0, 0).unwrap().id;
    let b = h.subscribe(0, 0).unwrap().id;
    assert_eq!(h.subscribe(0, 0).err(), Some(StreamError::Full));
    // A resubscribe keeps its slot, so it is never refused by the cap.
    h.resubscribe(b, 0, 0).unwrap();
    h.unsubscribe(a);
    assert_eq!(h.stream_count(), 1);
    h.subscribe(0, 0).unwrap();
}

#[test]
fn heartbeats_follow_the_schedule_of_the_caller_supplied_clock() {
    let mut h = H::new(cfg(), 0);
    let id = h.subscribe(0, 1000).unwrap().id;
    assert!(h.tick(1099).is_empty());
    assert!(drain(&mut h, id, 1099).is_empty());
    h.tick(1100);
    h.tick(1100); // a second tick does not queue a second heartbeat
    assert_eq!(drain(&mut h, id, 1100), vec![Frame::Heartbeat]);
    h.tick(1199);
    assert!(drain(&mut h, id, 1199).is_empty());
    h.tick(1200);
    assert_eq!(drain(&mut h, id, 1200), vec![Frame::Heartbeat]);
}

#[test]
fn a_frame_sent_counts_as_traffic_and_postpones_the_heartbeat() {
    let mut h = H::new(cfg(), 0);
    let id = h.subscribe(0, 0).unwrap().id;
    publish(&mut h, 1, 1);
    drain(&mut h, id, 90);
    h.tick(150);
    assert!(drain(&mut h, id, 150).is_empty());
    h.tick(190);
    assert_eq!(drain(&mut h, id, 190), vec![Frame::Heartbeat]);
}

#[test]
fn a_silent_client_is_dropped_after_the_dead_time_and_a_seen_one_is_not() {
    let mut h = H::new(cfg(), 0);
    let quiet = h.subscribe(0, 0).unwrap().id;
    let chatty = h.subscribe(0, 0).unwrap().id;
    h.seen(chatty, 300);
    assert_eq!(h.tick(350), vec![quiet]);
    assert_eq!(h.stream_count(), 1);
    assert!(h.poll(quiet, 350).is_none());
    assert!(h.tick(600).is_empty());
    assert_eq!(h.tick(650), vec![chatty]);
}

#[test]
fn heartbeats_tick_on_a_real_clock() {
    use std::time::{Duration, Instant};
    let start = Instant::now();
    let now = || u64::try_from(start.elapsed().as_millis()).unwrap();
    let mut h = H::new(
        Config {
            heartbeat_ms: 40,
            dead_after_ms: 10_000,
            ..cfg()
        },
        0,
    );
    let id = h.subscribe(0, now()).unwrap().id;
    h.tick(now());
    assert!(drain(&mut h, id, now()).is_empty(), "too early");
    std::thread::sleep(Duration::from_millis(60));
    h.tick(now());
    assert_eq!(drain(&mut h, id, now()), vec![Frame::Heartbeat]);
}

#[test]
fn a_skipped_number_resyncs_every_stream_and_the_store_covers_the_gap() {
    let mut h = H::new(cfg(), 0);
    let id = h.subscribe(0, 0).unwrap().id;
    publish(&mut h, 1, 2);
    assert_eq!(
        h.publish(5, "x".into()),
        Err(StreamError::Gap {
            expected: 3,
            got: 5
        })
    );
    let got = drain(&mut h, id, 0);
    assert_eq!(
        got.last(),
        Some(&Frame::Resync {
            after_seq: 2,
            latest_seq: 5
        })
    );
    let again = h.resubscribe(id, 2, 0).unwrap();
    assert_eq!(again.missing, Some((2, 5)));
}
