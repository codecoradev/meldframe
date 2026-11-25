//! The second check of a header (`RawHeader::check`) holds an actor, a key prefix and a tool to the same rule as every
//! other entry: `meldframe_text` (characters and bytes together), not a byte count of its own. A non-ASCII name that the
//! store and the tool take must not be dropped here, and one they refuse must not pass.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{MAX_ACTOR, MAX_KEY_PREFIX, MAX_TOOL, RawHeader};

fn passes(h: RawHeader) -> bool {
    h.check().is_some()
}

#[test]
fn a_key_prefix_is_counted_in_characters_and_bytes_like_the_store_and_the_tool_do() {
    // 100 characters of 2, 3 and 4 bytes: all fit (at most 100 characters and 400 bytes).
    for c in ['\u{00E9}', '\u{4E2D}', '\u{1F600}'] {
        let prefix = c.to_string().repeat(MAX_KEY_PREFIX);
        assert!(passes(header(1, 1, "agent:a", &prefix, 1, 5)), "{c}");
        let longer = c.to_string().repeat(MAX_KEY_PREFIX + 1);
        assert!(!passes(header(1, 1, "agent:a", &longer, 1, 5)), "{c} + 1");
    }
}

#[test]
fn an_actor_is_at_most_128_bytes_so_128_non_ascii_characters_are_too_long() {
    assert!(passes(header(1, 1, &"a".repeat(MAX_ACTOR), "b1", 1, 5)));
    // 64 two-byte characters are 128 bytes; 128 of them are 256.
    assert!(passes(header(1, 1, &"\u{00E9}".repeat(64), "b1", 1, 5)));
    assert!(!passes(header(1, 1, &"\u{00E9}".repeat(128), "b1", 1, 5)));
    assert!(!passes(header(1, 1, &"\u{4E2D}".repeat(43), "b1", 1, 5)));
}

#[test]
fn a_tool_is_at_most_64_bytes() {
    let mut h = header(1, 1, "agent:a", "b1", 1, 5);
    h.tool = "\u{00E9}".repeat(MAX_TOOL / 2);
    assert!(passes(h.clone()));
    h.tool = "\u{00E9}".repeat(MAX_TOOL / 2 + 1);
    assert!(!passes(h));
}

#[test]
fn a_run_of_stacked_marks_in_a_name_is_refused_as_the_store_and_the_tool_refuse_it() {
    let stacked = format!("agent:e{}", "\u{0301}".repeat(5));
    assert!(!passes(header(1, 1, &stacked, "b1", 1, 5)));
    let stacked = format!("b{}", "\u{0301}".repeat(5));
    assert!(!passes(header(1, 1, "agent:a", &stacked, 1, 5)));
    let ok = format!("agent:e{}", "\u{0301}".repeat(4));
    assert!(passes(header(1, 1, &ok, "b1", 1, 5)));
}

#[test]
fn a_reviewer_is_held_to_the_actor_rule_and_goes_with_its_time_when_it_fails() {
    let mut h = header(1, 1, "agent:a", "b1", 1, 5);
    h.reviewed_by = Some("\u{00E9}".repeat(128));
    h.reviewed_at_ms = Some(9);
    let checked = h.check().expect("the header stays");
    assert_eq!((checked.reviewed_by, checked.reviewed_at_ms), (None, None));
}
