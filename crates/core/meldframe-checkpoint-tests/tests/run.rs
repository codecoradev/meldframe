//! The run rule (ADR-0023): when the server takes a checkpoint on its own, and the one constant the lane, the app
//! and the server share.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_checkpoints::{RunWatch, is_agent, seq_before_first};
use meldframe_revert::{RUN_GAP_MS, starts_run};

const GAP: u64 = 300_000;
const T0: u64 = 1_700_000_000_000;

fn start(w: &mut RunWatch, actor: &str, at: u64) -> bool {
    w.starts("ws", "file", actor, at)
}

#[test]
fn the_gap_is_five_minutes_and_is_written_once() {
    assert_eq!(RUN_GAP_MS, GAP);
    // The lane's run (meldframe-activity) reads the same constant.
    assert_eq!(meldframe_activity::RUN_GAP_MS, RUN_GAP_MS);
}

#[test]
fn a_gap_of_exactly_five_minutes_is_not_a_start_and_one_millisecond_more_is() {
    let mut w = RunWatch::new();
    assert!(
        start(&mut w, "agent:a", T0),
        "the first op seen starts a run"
    );
    assert!(!start(&mut w, "agent:a", T0 + GAP));
    // The last op is now T0 + GAP: the next gap is counted from it.
    assert!(!start(&mut w, "agent:a", T0 + GAP + GAP));
    assert!(start(&mut w, "agent:a", T0 + GAP + GAP + GAP + 1));
}

#[test]
fn a_call_inside_the_gap_takes_nothing() {
    let mut w = RunWatch::new();
    assert!(start(&mut w, "agent:a", T0));
    for i in 1..20 {
        assert!(!start(&mut w, "agent:a", T0 + i * 10_000));
    }
}

#[test]
fn a_clock_that_goes_back_is_the_same_run_and_cannot_make_a_later_call_a_gap() {
    let mut w = RunWatch::new();
    assert!(start(&mut w, "agent:a", T0));
    assert!(!start(&mut w, "agent:a", T0 - 10 * GAP));
    // The time kept is the larger, so the clock coming back to a later time is still inside the run, counted from
    // there; kept as the last time given, it would be a gap of more than five minutes from the time it stepped back to.
    assert!(!start(&mut w, "agent:a", T0 + 1));
    assert!(!start(&mut w, "agent:a", T0 + 1 + GAP));
    assert!(start(&mut w, "agent:a", T0 + 1 + 2 * GAP + 1));
}

#[test]
fn the_pure_rule_is_what_the_detector_uses() {
    assert!(starts_run(None, T0, GAP));
    assert!(!starts_run(Some(T0), T0 + GAP, GAP));
    assert!(starts_run(Some(T0), T0 + GAP + 1, GAP));
    assert!(!starts_run(Some(T0), 0, GAP));
    assert!(!starts_run(Some(u64::MAX), 0, GAP));
    assert!(starts_run(Some(0), u64::MAX, GAP));
}

#[test]
fn only_an_agent_starts_a_run_and_a_person_leaves_no_trace() {
    let mut w = RunWatch::new();
    for actor in [
        "person:ana",
        "ana",
        "",
        "Agent:x",
        " agent:x",
        "agents:x",
        "agent",
    ] {
        assert!(!is_agent(actor), "{actor:?}");
        assert!(!start(&mut w, actor, T0), "{actor:?}");
        assert!(!start(&mut w, actor, T0 + 10 * GAP), "{actor:?}");
    }
    assert_eq!(w.len(), 0, "a person is never recorded");
    assert!(is_agent("agent:claude"));
    assert!(is_agent("agent:"));
}

#[test]
fn a_run_belongs_to_one_actor_in_one_file_of_one_workspace() {
    let mut w = RunWatch::new();
    assert!(w.starts("ws", "f1", "agent:a", T0));
    assert!(w.starts("ws", "f2", "agent:a", T0 + 1), "another file");
    assert!(
        w.starts("ws2", "f1", "agent:a", T0 + 2),
        "another workspace"
    );
    assert!(w.starts("ws", "f1", "agent:b", T0 + 3), "another actor");
    assert!(!w.starts("ws", "f1", "agent:a", T0 + 4));
    assert!(!w.starts("ws", "f2", "agent:a", T0 + 4));
    // Names that only look alike once joined are not the same key.
    assert!(w.starts("w", "sf1", "agent:a", T0 + 5));
}

#[test]
fn an_entry_older_than_the_gap_is_dropped_and_gives_the_same_answer() {
    let mut w = RunWatch::new();
    for i in 0..50 {
        assert!(w.starts("ws", &format!("f{i}"), "agent:a", T0 + i));
    }
    assert_eq!(w.len(), 50);
    // One call later than every entry by more than the gap prunes all of them, and then holds only itself.
    assert!(start(&mut w, "agent:b", T0 + 100 + GAP + 1));
    assert_eq!(w.len(), 1);
    // Dropped and never seen give the same answer.
    assert!(w.starts("ws", "f3", "agent:a", T0 + 100 + GAP + 2));
}

#[test]
fn the_map_is_capped_and_the_oldest_entry_goes() {
    let mut w = RunWatch::with_limits(GAP, 3);
    for (i, f) in ["f1", "f2", "f3"].iter().enumerate() {
        assert!(w.starts("ws", f, "agent:a", T0 + i as u64));
    }
    assert_eq!(w.len(), 3);
    assert!(w.starts("ws", "f4", "agent:a", T0 + 10));
    assert_eq!(w.len(), 3, "at the cap the oldest entry went");
    // f1 was the oldest: it starts a run again. The ones that stayed do not.
    assert!(!w.starts("ws", "f3", "agent:a", T0 + 11));
    assert!(w.starts("ws", "f1", "agent:a", T0 + 12));
    assert_eq!(w.len(), 3);
    // An agent that is seen again takes no new place.
    let mut w = RunWatch::with_limits(GAP, 2);
    assert!(w.starts("ws", "f1", "agent:a", T0));
    assert!(w.starts("ws", "f2", "agent:a", T0 + 1));
    assert!(!w.starts("ws", "f1", "agent:a", T0 + 2));
    assert_eq!(w.len(), 2);
    assert!(!w.starts("ws", "f2", "agent:a", T0 + 3));
}

#[test]
fn the_default_cap_is_ten_thousand_entries() {
    assert_eq!(meldframe_checkpoints::MAX_DETECTOR_ENTRIES, 10_000);
    let mut w = RunWatch::new();
    for i in 0..10_050u64 {
        w.starts("ws", &format!("f{i}"), "agent:a", T0 + i);
    }
    assert_eq!(w.len(), 10_000);
}

#[test]
fn a_run_starts_just_before_the_first_op_it_wrote() {
    assert_eq!(seq_before_first([7, 5, 9]), Some(4));
    assert_eq!(seq_before_first([1]), Some(0));
    assert_eq!(seq_before_first([0]), Some(0));
    assert_eq!(seq_before_first(Vec::new()), None);
}
