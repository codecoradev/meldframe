//! The in-memory checkpoint store against the shared contract, one test per case.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use checkpoint_store::{MemoryCheckpoints, contract};

macro_rules! case {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            contract::$name(&MemoryCheckpoints::default());
        }
    )*};
}

case!(
    numbers_start_at_one_per_file_and_a_list_is_newest_first,
    a_row_is_kept_as_it_was_made,
    a_retried_key_returns_its_row_and_a_different_request_is_key_reused,
    a_key_belongs_to_one_actor_in_one_file_of_one_workspace,
    a_key_that_rolled_out_of_the_file_makes_a_second_row,
    a_run_start_is_unique_per_actor_and_seq,
    a_manual_row_is_refused_at_its_share_and_at_its_actors_cap_and_never_rolled,
    automatic_rows_roll_inside_their_share_and_never_cross_kinds,
    the_before_kinds_roll_the_oldest_of_either,
    an_agent_that_starts_runs_all_day_only_replaces_its_own_rows,
    another_workspaces_file_is_empty,
    input_is_checked_before_anything_is_written,
);
