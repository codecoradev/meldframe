//! The review enum, the header types and the bounds (ADR-0021).

use meldframe_activity::{
    DEFAULT_PAGE, MAX_ACTOR, MAX_COUNT, MAX_KEY_PREFIX, MAX_PAGE, MAX_PER_ACTOR, MAX_PER_FILE,
    MAX_PER_WORKSPACE, MAX_TOOL, Review,
};

#[test]
fn the_three_states_have_their_stable_names_and_round_trip() {
    for (r, name) in [
        (Review::Unreviewed, "unreviewed"),
        (Review::Accepted, "accepted"),
        (Review::NeedsReview, "needs_review"),
    ] {
        assert_eq!(r.as_str(), name);
        assert_eq!(Review::parse(name), Some(r));
    }
}

#[test]
fn any_other_text_is_not_a_state() {
    for t in [
        "",
        "Accepted",
        "ACCEPTED",
        "needsReview",
        "needs-review",
        " accepted",
        "accepted ",
        "rejected",
        "reviewed",
    ] {
        assert_eq!(Review::parse(t), None, "{t:?}");
    }
}

#[test]
fn a_new_header_is_unreviewed_and_only_accepted_leaves_the_count() {
    assert_eq!(Review::default(), Review::Unreviewed);
    assert!(Review::Unreviewed.is_open());
    assert!(Review::NeedsReview.is_open());
    assert!(!Review::Accepted.is_open());
}

#[test]
fn the_bounds_are_the_ones_of_the_adr() {
    assert_eq!(MAX_COUNT, 100_000);
    assert_eq!(
        (MAX_PER_ACTOR, MAX_PER_FILE, MAX_PER_WORKSPACE),
        (2_000, 20_000, 200_000)
    );
    assert_eq!((DEFAULT_PAGE, MAX_PAGE), (50, 100));
    assert_eq!((MAX_ACTOR, MAX_TOOL), (128, 64));
}

#[test]
fn the_key_prefix_bound_is_the_idempotency_key_of_the_tools() {
    assert_eq!(MAX_KEY_PREFIX, meldframe_mcp_defs::MAX_KEY);
}
