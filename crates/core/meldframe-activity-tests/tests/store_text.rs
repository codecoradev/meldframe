//! What the batch store (`activity-store`, and so `meldframe-store-pg`, which calls the same checks) refuses before it
//! writes, for text that is not ASCII: one rule at every entry (`meldframe_text`), so the store, the `get_batches` tool and
//! the app layer's second check agree on a name that is 128 characters of two bytes, or 100 characters of four.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use activity_store::{ActivityError, BatchStore, MemoryBatches, check_actor, check_batch};
use meldframe_activity::{MAX_ACTOR, MAX_KEY_PREFIX, NewBatch};

fn batch(actor: &str, prefix: &str) -> NewBatch {
    NewBatch {
        actor: actor.into(),
        key_prefix: prefix.into(),
        tool: "apply_commands".into(),
        commands: 1,
        written: 1,
        known: None,
        atomic: true,
        first_seq: 1,
        last_seq: 1,
        at_ms: 1,
        note: None,
    }
}

fn refused(b: &NewBatch) -> bool {
    matches!(check_batch(b), Err(ActivityError::Invalid(_)))
}

#[test]
fn an_actor_is_at_most_128_bytes_whatever_the_script() {
    assert!(!refused(&batch(&"a".repeat(MAX_ACTOR), "k")));
    // 64 two-byte characters are 128 bytes; 128 of them are 256, and 43 three-byte ones are 129.
    assert!(!refused(&batch(&"\u{00E9}".repeat(64), "k")));
    assert!(refused(&batch(&"\u{00E9}".repeat(MAX_ACTOR), "k")));
    assert!(refused(&batch(&"\u{4E2D}".repeat(43), "k")));
    assert!(check_actor(&"\u{4E2D}".repeat(43)).is_err());
}

#[test]
fn a_key_prefix_is_at_most_100_characters_and_400_bytes_whatever_the_script() {
    for c in ['\u{00E9}', '\u{4E2D}', '\u{1F600}'] {
        let ok = c.to_string().repeat(MAX_KEY_PREFIX);
        assert!(!refused(&batch("agent:a", &ok)), "{c}");
        let long = c.to_string().repeat(MAX_KEY_PREFIX + 1);
        assert!(refused(&batch("agent:a", &long)), "{c} + 1");
    }
}

#[test]
fn stacked_marks_in_an_actor_or_a_key_prefix_are_refused_like_everywhere_else() {
    let stacked = "\u{0301}".repeat(5);
    assert!(refused(&batch(&format!("agent:e{stacked}"), "k")));
    assert!(refused(&batch("agent:a", &format!("k{stacked}"))));
    let four = "\u{0301}".repeat(4);
    assert!(!refused(&batch(&format!("agent:e{four}"), "k")));
}

#[test]
fn the_longest_non_ascii_names_are_recorded() {
    let s = MemoryBatches::default();
    let wide = batch(&"\u{00E9}".repeat(64), &"\u{1F600}".repeat(MAX_KEY_PREFIX));
    assert!(s.record("ws", "f", &wide).is_ok());
}
