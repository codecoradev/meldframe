//! U9: the text rules of the MCP surface, through a real client. An idempotency key and a file name are held to the
//! one set of hidden characters (`meldframe-text`), by one rule in every tool.

#[path = "mcp_common/mod.rs"]
mod common;
use common::*;
use serde_json::{Value, json};

/// What the old checks let through (a control character was the whole rule of a key) and what only some of them
/// refused: a right-to-left override, a zero-width space and joiner, a line separator, the word joiner, the combining
/// grapheme joiner, the Hangul filler, a tag character.
const HIDDEN: [char; 8] = [
    '\u{202E}',
    '\u{200B}',
    '\u{200D}',
    '\u{2028}',
    '\u{2060}',
    '\u{034F}',
    '\u{3164}',
    '\u{E0041}',
];

fn add(id: &str) -> Value {
    frame(id, id)
}

async fn apply(c: &Client, file: &str, key: &str) -> Called {
    call(
        c,
        "apply_commands",
        json!({"file": file, "idempotencyKey": key, "commands": [add("a")]}),
    )
    .await
}

async fn seq(c: &Client, file: &str) -> u64 {
    call(c, "get_tree", json!({"file": file})).await.body["seq"]
        .as_u64()
        .unwrap()
}

#[tokio::test]
async fn a_key_with_a_hidden_character_is_refused_and_nothing_is_written() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    for ch in HIDDEN {
        let r = apply(&c, "k1", &format!("run{ch}1")).await;
        assert!(r.is_error, "U+{:04X}: {}", ch as u32, r.body);
        assert_eq!(r.body["error"]["code"], "invalid", "U+{:04X}", ch as u32);
        let r = call(
            &c,
            "batch_status",
            json!({"file": "k1", "idempotencyKey": format!("run{ch}1")}),
        )
        .await;
        assert!(r.is_error, "batch_status U+{:04X}: {}", ch as u32, r.body);
    }
    assert_eq!(seq(&c, "k1").await, 0, "a refused key wrote a command");
}

#[tokio::test]
async fn a_key_of_100_characters_is_taken_in_ascii_and_in_four_byte_text_and_101_is_not() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let ascii = "k".repeat(100);
    let emoji = "\u{1F600}".repeat(100);
    let accented = "\u{e9}".repeat(100);
    for (i, key) in [ascii, emoji, accented].iter().enumerate() {
        let file = format!("k2{i}");
        let r = apply(&c, &file, key).await;
        assert!(!r.is_error, "key {i}: {}", r.body);
        assert_eq!(seq(&c, &file).await, 1, "key {i}");
    }
    let r = apply(&c, "k23", &"\u{1F600}".repeat(101)).await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "invalid");
    assert_eq!(seq(&c, "k23").await, 0);
}

#[tokio::test]
async fn a_file_name_with_a_hidden_character_is_refused_by_every_tool_that_takes_one() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    for ch in HIDDEN {
        let name = format!("page{ch}");
        let r = apply(&c, &name, "k").await;
        assert!(r.is_error, "apply U+{:04X}: {}", ch as u32, r.body);
        assert_eq!(r.body["error"]["code"], "invalid", "U+{:04X}", ch as u32);
        let r = call(&c, "get_tree", json!({"file": name})).await;
        assert!(r.is_error, "get_tree U+{:04X}: {}", ch as u32, r.body);
    }
    // A name of 128 bytes is the longest; 65 two-byte characters are 130 bytes.
    let r = call(&c, "get_tree", json!({"file": "\u{e9}".repeat(65)})).await;
    assert!(r.is_error, "{}", r.body);
    let r = call(&c, "get_tree", json!({"file": "\u{e9}".repeat(64)})).await;
    assert!(!r.is_error, "{}", r.body);
}

#[tokio::test]
async fn a_run_of_more_than_4_combining_marks_is_refused_in_a_key_and_a_file_name() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let marks = |n: usize| "\u{0301}".repeat(n);
    // Four marks in a row are taken, five are not, in a key and in a file name alike.
    let r = apply(&c, "m1", &format!("k{}", marks(4))).await;
    assert!(!r.is_error, "{}", r.body);
    let r = apply(&c, "m2", &format!("k{}", marks(5))).await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "invalid");
    assert_eq!(seq(&c, "m2").await, 0, "a refused key wrote a command");
    let r = apply(&c, &format!("f{}", marks(4)), "k").await;
    assert!(!r.is_error, "{}", r.body);
    let r = apply(&c, &format!("f{}", marks(5)), "k").await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "invalid");
}
