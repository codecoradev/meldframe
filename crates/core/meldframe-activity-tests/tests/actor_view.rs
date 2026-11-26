//! `ActorView` (#320, finding T2 of the thin-UI audit): the kind, the name and the tone an actor wears, derived once.
//! The tone hashes the code points of the whole actor, so a character outside the Basic Multilingual Plane counts once
//! (the two derivations of the web client counted UTF-16 units in one place and the first unit of each code point in
//! the other, and gave one actor two colours).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_activity::{ActorView, LaneKind, Tone};

/// The hash of the audit, written out with no shortcut: `h = h * 31 + code point` modulo 2^32, then `h % 3`.
fn tone_of(actor: &str) -> Tone {
    let mut h: u64 = 0;
    for c in actor.chars() {
        h = (h * 31 + u64::from(u32::from(c))) % (1 << 32);
    }
    [Tone::Agent1, Tone::Agent2, Tone::Agent3][(h % 3) as usize]
}

#[test]
fn an_agent_is_told_by_its_prefix_and_named_by_what_follows() {
    let v = ActorView::of("agent:claude");
    assert_eq!(v.actor, "agent:claude");
    assert_eq!((v.kind, v.name.as_str()), (LaneKind::Agent, "claude"));
    assert_eq!(v.tone, tone_of("agent:claude"));
}

#[test]
fn a_person_has_one_tone_and_so_has_an_actor_of_no_known_kind() {
    let v = ActorView::of("person:ana");
    assert_eq!(
        (v.kind, v.name.as_str(), v.tone),
        (LaneKind::Person, "ana", Tone::Person)
    );
    // No known kind: a person, named by the whole text.
    let v = ActorView::of("user:alice");
    assert_eq!(
        (v.kind, v.name.as_str(), v.tone),
        (LaneKind::Person, "user:alice", Tone::Person)
    );
    let v = ActorView::of("ana");
    assert_eq!((v.kind, v.name.as_str()), (LaneKind::Person, "ana"));
}

#[test]
fn a_name_with_nothing_after_the_prefix_is_the_actor() {
    let v = ActorView::of("agent:");
    assert_eq!((v.kind, v.name.as_str()), (LaneKind::Agent, "agent:"));
}

#[test]
fn the_tone_counts_code_points_so_a_character_outside_the_bmp_counts_once() {
    for actor in [
        "agent:🌀",
        "agent:😀x",
        "agent:🌀🌀",
        "agent:ünï",
        "agent:a",
    ] {
        assert_eq!(ActorView::of(actor).tone, tone_of(actor), "{actor}");
    }
    // The values of the audit's two examples, so that a change of the hash shows in a diff.
    assert_eq!(ActorView::of("agent:🌀").tone, Tone::Agent3);
    assert_eq!(ActorView::of("agent:😀x").tone, Tone::Agent1);
}

#[test]
fn the_three_agent_tones_are_all_used() {
    let tones: std::collections::BTreeSet<_> = ["a", "b", "c", "d", "e", "f"]
        .iter()
        .map(|s| format!("{:?}", ActorView::of(&format!("agent:{s}")).tone))
        .collect();
    assert_eq!(tones.len(), 3, "{tones:?}");
}
