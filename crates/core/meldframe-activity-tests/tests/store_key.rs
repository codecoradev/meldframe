//! `store_key`: the key of a file's rows in the in-memory stores of the activity group (batch headers and checkpoints).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_activity::store_key;

#[test]
fn a_key_is_the_workspace_and_the_file_and_nothing_else() {
    assert_eq!(store_key("ws", "f"), ("ws".to_owned(), "f".to_owned()));
    assert_eq!(store_key("", ""), (String::new(), String::new()));
}

#[test]
fn the_same_text_split_at_another_place_is_another_key() {
    // A workspace never reaches another workspace's file by the way its name runs into the file's.
    assert_ne!(store_key("a/b", "c"), store_key("a", "b/c"));
    assert_ne!(store_key("ab", ""), store_key("a", "b"));
    assert_ne!(store_key("w", "f"), store_key("f", "w"));
}
