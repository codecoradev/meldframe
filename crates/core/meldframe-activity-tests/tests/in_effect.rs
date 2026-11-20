//! `in_effect` and `revertable`: would `revert_batch` still change anything (ADR-0021, "Status").

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{InEffect, InEffectMemo, in_effect, revertable};
use meldframe_revert::ConfirmedLog;

#[test]
fn a_batch_is_in_effect_until_every_field_it_wrote_was_written_again() {
    let before = file_before();
    let mut log = vec![op(1, "agent:a", "k:0", set_x("a", 50.0))];
    assert_eq!(in_effect(&before, &log, "agent:a", "k"), InEffect::Yes);
    // The person wrote the same field afterwards: nothing is left to take back.
    log.push(op(2, ME, "ed-1", set_x("a", 70.0)));
    assert_eq!(in_effect(&before, &log, "agent:a", "k"), InEffect::No);
}

#[test]
fn a_batch_that_was_reverted_is_no_longer_in_effect() {
    let before = file_before();
    let mut log = vec![op(1, "agent:a", "k:0", set_x("a", 50.0))];
    // The person's revert writes the old value back.
    log.push(op(2, ME, "ed-rv", set_x("a", 0.0)));
    assert_eq!(in_effect(&before, &log, "agent:a", "k"), InEffect::No);
}

#[test]
fn a_batch_the_log_does_not_hold_or_a_log_that_does_not_replay_is_unknown() {
    let before = file_before();
    let log = vec![op(1, "agent:a", "k:0", set_x("a", 50.0))];
    assert_eq!(
        in_effect(&before, &log, "agent:a", "other"),
        InEffect::Unknown
    );
    assert_eq!(in_effect(&before, &log, "agent:b", "k"), InEffect::Unknown);
    let broken = vec![op(1, "agent:a", "k:0", set_x("missing", 1.0))];
    assert_eq!(
        in_effect(&before, &broken, "agent:a", "k"),
        InEffect::Unknown
    );
}

#[test]
fn the_answer_is_kept_by_confirmed_seq_and_batch() {
    let before = file_before();
    let log = vec![op(1, "agent:a", "k:0", set_x("a", 50.0))];
    let mut memo = InEffectMemo::default();
    let mut runs = 0;
    for seq in [1, 1, 1, 2] {
        let got = memo.get(seq, "agent:a", "k", || {
            runs += 1;
            in_effect(&before, &log, "agent:a", "k")
        });
        assert_eq!(got, InEffect::Yes);
    }
    assert_eq!(runs, 2);
    memo.get(2, "agent:a", "k2", || {
        runs += 1;
        InEffect::No
    });
    assert_eq!(runs, 3);
}

#[test]
fn a_batch_is_revertable_only_while_the_record_holds_all_of_it() {
    let mut rec = ConfirmedLog::new(file_before(), 0);
    assert!(!revertable(&rec, "agent:a", "k"));
    rec.push(op(1, "agent:a", "k:0", set_x("a", 1.0)));
    assert!(revertable(&rec, "agent:a", "k"));
    assert!(!revertable(&rec, "agent:b", "k"));
    // Fold it into the base: the batch is partial, then gone.
    for i in 0..(2_000 + 256) {
        rec.push(op(2 + i, ME, &format!("ed-{i}"), set_x("a", i as f64)));
    }
    assert!(!revertable(&rec, "agent:a", "k"));
}
