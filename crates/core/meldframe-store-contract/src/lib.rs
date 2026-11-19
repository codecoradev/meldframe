//! The contract of the `Store` seam as test cases that every adapter runs.
//!
//! An adapter's test file calls [`store_contract!`] with a function that returns a fresh, empty store, or
//! `None` when its backend is not available (a Postgres adapter without `DATABASE_URL`): the cases then
//! skip, loudly, instead of failing, unless `MELDFRAME_REQUIRE_PG` is set, which makes a missing backend a
//! failure (CI sets it). Each case gets its own store, so cases never see each other's rooms.
//!
//! ```ignore
//! fn make() -> Option<MemoryStore> { Some(MemoryStore::new()) }
//! meldframe_store_contract::store_contract!(make);
//! ```

mod cases;

pub use cases::*;
pub use meldframe_host;

/// Generate one `#[test]` per contract case, each against a store from `$make` (`fn() -> Option<impl Store>`).
#[macro_export]
macro_rules! store_contract {
    ($make:path) => {
        $crate::store_contract!(@each $make,
            sequence_numbers_start_at_one_and_count_per_room,
            a_retried_key_returns_the_same_sequence_number_and_writes_nothing,
            a_key_belongs_to_one_actor_in_one_room,
            ops_after_returns_the_log_in_order_with_actor_and_key,
            a_limit_smaller_than_the_tail_returns_the_first_page_in_order,
            replay_through_pages_equals_replay_in_one_go,
            there_is_no_snapshot_until_one_is_saved_and_the_newest_wins,
            a_snapshot_ahead_of_the_log_or_older_than_the_last_is_refused,
            a_snapshot_never_changes_the_log,
            a_key_reused_for_a_different_command_is_refused_and_writes_nothing,
            a_snapshot_must_be_newer_than_the_latest_and_a_failed_save_keeps_the_old_one,
            a_snapshot_restores_with_the_log_after_it,
            a_command_comes_back_from_the_log_unchanged,
            a_snapshot_comes_back_unchanged,
            variables_come_back_with_the_snapshot_and_the_log
        );
    };
    (@each $make:path, $($case:ident),+) => {
        $(
            #[test]
            fn $case() {
                match $make() {
                    Some(mut store) => $crate::$case(&mut store),
                    None if ::std::env::var_os("MELDFRAME_REQUIRE_PG").is_some() => {
                        panic!("MELDFRAME_REQUIRE_PG is set but the store backend is not available")
                    }
                    None => eprintln!("SKIPPED {}: the store backend is not available", stringify!($case)),
                }
            }
        )+
    };
}
