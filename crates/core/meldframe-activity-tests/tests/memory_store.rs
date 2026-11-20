//! The in-memory batch store against the shared contract, one test per case.

use activity_store::{Caps, MemoryBatches, contract};
use meldframe_activity::{MAX_PER_ACTOR, MAX_PER_FILE, MAX_PER_WORKSPACE};

macro_rules! case {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            contract::$name(&MemoryBatches::default());
        }
    )*};
}

case!(
    numbers_start_at_one_per_file_and_are_never_reused,
    a_batch_is_one_header_per_actor_and_key_prefix,
    rev_rises_on_every_change_and_not_on_a_no_op,
    an_update_never_lowers_and_may_raise_what_it_keeps,
    commands_never_fall_below_written_across_the_parts_of_an_import,
    the_first_note_wins_and_the_answer_says_which,
    a_review_is_set_by_a_person_and_survives_a_later_record,
    a_review_of_a_missing_batch_or_one_of_another_workspace_is_not_found,
    the_unreviewed_count_is_every_state_but_accepted,
    a_list_is_newest_first_with_before_after_and_has_more,
    input_is_checked_before_anything_is_written,
);

#[test]
fn the_caps_refuse_a_new_header_and_never_an_update() {
    contract::the_caps_refuse_a_new_header_and_never_an_update(&MemoryBatches::with_caps(
        contract::small_caps(),
    ));
}

#[test]
fn the_default_caps_are_the_bounds_of_the_adr() {
    let caps = Caps::default();
    assert_eq!(
        (caps.per_actor, caps.per_file, caps.per_workspace),
        (MAX_PER_ACTOR, MAX_PER_FILE, MAX_PER_WORKSPACE)
    );
}

#[test]
fn one_runaway_actor_stops_at_2000_and_leaves_the_rest_of_the_file_its_numbers() {
    use activity_store::{ActivityError, BatchStore};
    let s = MemoryBatches::default();
    for i in 0..MAX_PER_ACTOR {
        s.record(
            "ws",
            "f",
            &contract::batch("agent:loop", &format!("k{i}"), 1, 1),
        )
        .unwrap();
    }
    let refused = s.record("ws", "f", &contract::batch("agent:loop", "more", 1, 1));
    assert!(
        matches!(refused, Err(ActivityError::Full { limit: 2_000, .. })),
        "{refused:?}"
    );
    let other = s
        .record("ws", "f", &contract::batch("agent:other", "k", 1, 1))
        .unwrap();
    assert_eq!(other.header.n as usize, MAX_PER_ACTOR + 1);
}

#[test]
fn the_file_and_workspace_caps_hold_at_their_real_sizes() {
    use activity_store::{ActivityError, BatchStore};
    let s = MemoryBatches::default();
    // 10 actors of 2,000 fill a file of 20,000; the next actor is refused by the file cap.
    for a in 0..10 {
        for i in 0..MAX_PER_ACTOR {
            let b = contract::batch(&format!("agent:{a}"), &format!("k{i}"), 1, 1);
            s.record("ws", "f", &b).unwrap();
        }
    }
    let one = contract::batch("agent:11", "k", 1, 1);
    let refused = s.record("ws", "f", &one);
    assert!(
        matches!(refused, Err(ActivityError::Full { limit: 20_000, .. })),
        "{refused:?}"
    );
    // Nine more files fill the workspace to 200,000; the next header of a tenth is refused by the workspace cap.
    for f in 0..9 {
        for a in 0..10 {
            for i in 0..MAX_PER_ACTOR {
                let b = contract::batch(&format!("agent:{a}"), &format!("k{i}"), 1, 1);
                s.record("ws", &format!("g{f}"), &b).unwrap();
            }
        }
    }
    let refused = s.record("ws", "h", &one);
    assert!(
        matches!(refused, Err(ActivityError::Full { limit: 200_000, .. })),
        "{refused:?}"
    );
    assert!(s.record("other", "h", &one).is_ok());
}

#[test]
fn error_codes_doc_lists_exactly_the_codes() {
    let doc = include_str!("../../../../docs/error-codes.md");
    let section = doc.split("\n## Activity codes\n").nth(1).expect("section");
    let section = section.split("\n## ").next().unwrap();
    let mut listed: Vec<&str> = section
        .lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| l[3..].split('`').next().unwrap())
        .collect();
    let mut codes = activity_store::CODES.to_vec();
    listed.sort_unstable();
    codes.sort_unstable();
    assert_eq!(listed, codes);
}
