//! The refusing checks: `plain`, the actor, the key and the names, each with the characters that went wrong before.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_text::{
    AGENT_PREFIX, MAX_ACTOR, MAX_FILE_NAME, MAX_KEY, MAX_KEY_BYTES, TextError, actor, file_name,
    is_agent, key, plain, workspace_name,
};

#[test]
fn plain_refuses_empty_long_hidden_and_smeared_text_in_that_order() {
    assert_eq!(plain("", 5, 5), Err(TextError::Empty));
    assert_eq!(plain("abcdef", 5, 100), Err(TextError::TooManyChars(5)));
    assert_eq!(plain("abcdef", 100, 5), Err(TextError::TooManyBytes(5)));
    assert_eq!(
        plain("a\u{202E}b", 5, 5),
        Err(TextError::Hidden('\u{202E}'))
    );
    assert_eq!(
        plain("a\u{2028}b", 5, 9),
        Err(TextError::Hidden('\u{2028}'))
    );
    assert_eq!(
        plain("a\u{200D}b", 5, 9),
        Err(TextError::Hidden('\u{200D}'))
    );
    assert_eq!(plain("a\nb", 5, 5), Err(TextError::Hidden('\n')));
    let smeared = "a".to_owned() + &"\u{0301}".repeat(5);
    assert_eq!(plain(&smeared, 20, 20), Err(TextError::TooManyMarks));
    assert_eq!(plain("abcde", 5, 5), Ok(()));
    // Over both caps, the byte count is the one reported (it is checked first, and read without walking the text).
    let both = "\u{1F600}".repeat(101);
    assert_eq!(plain(&both, 100, 100), Err(TextError::TooManyBytes(100)));
}

#[test]
fn both_counts_are_checked_for_text_that_is_not_ascii() {
    // Four-byte characters at the cap: the character count passes and the byte count does not.
    let at_chars = "\u{1F600}".repeat(5);
    assert_eq!(plain(&at_chars, 5, 20), Ok(()));
    assert_eq!(plain(&at_chars, 5, 19), Err(TextError::TooManyBytes(19)));
    assert_eq!(plain(&at_chars, 4, 20), Err(TextError::TooManyChars(4)));
}

#[test]
fn a_128_character_actor_that_is_not_ascii_fails_the_same_everywhere() {
    // The actor is bounded in bytes (a column holds no more), so 128 two-byte characters are refused, and so are 65.
    let long = "\u{e9}".repeat(MAX_ACTOR);
    assert_eq!(actor(&long), Err(TextError::TooManyBytes(MAX_ACTOR)));
    assert_eq!(actor(&"\u{e9}".repeat(64)), Ok(()));
    assert_eq!(
        actor(&"\u{e9}".repeat(65)),
        Err(TextError::TooManyBytes(MAX_ACTOR))
    );
    assert_eq!(actor(&"a".repeat(MAX_ACTOR)), Ok(()));
    assert_eq!(
        actor(&"a".repeat(MAX_ACTOR + 1)),
        Err(TextError::TooManyBytes(MAX_ACTOR))
    );
    // Four-byte characters: 32 fit, 33 do not.
    assert_eq!(actor(&"\u{1F600}".repeat(32)), Ok(()));
    assert!(actor(&"\u{1F600}".repeat(33)).is_err());
    assert_eq!(actor("agent:dev"), Ok(()));
    assert_eq!(
        actor("agent:\u{202E}dev"),
        Err(TextError::Hidden('\u{202E}'))
    );
    assert_eq!(actor(""), Err(TextError::Empty));
}

#[test]
fn a_key_is_counted_in_characters_with_a_byte_backstop() {
    assert_eq!(key(&"k".repeat(MAX_KEY)), Ok(()));
    assert_eq!(
        key(&"k".repeat(MAX_KEY + 1)),
        Err(TextError::TooManyChars(MAX_KEY))
    );
    // 100 characters of four bytes are 400 bytes: inside the backstop; 100 two-byte characters pass too.
    assert_eq!(key(&"\u{1F600}".repeat(MAX_KEY)), Ok(()));
    assert_eq!("\u{1F600}".repeat(MAX_KEY).len(), MAX_KEY_BYTES);
    assert_eq!(key(&"\u{e9}".repeat(MAX_KEY)), Ok(()));
    // The old tool check let these through: a right-to-left override, a zero-width space, a line separator.
    for c in ['\u{202E}', '\u{200B}', '\u{2028}', '\u{200D}', '\u{FEFF}'] {
        assert_eq!(
            key(&format!("k{c}k")),
            Err(TextError::Hidden(c)),
            "U+{:04X}",
            c as u32
        );
    }
    assert_eq!(key(""), Err(TextError::Empty));
    assert_eq!(key("run-1:\u{4E2D}"), Ok(()));
}

#[test]
fn names_follow_one_rule_with_a_dot_rule_for_a_file() {
    assert_eq!(file_name("Landing page"), Ok(()));
    assert_eq!(file_name("\u{4E2D}\u{6587}"), Ok(()));
    assert_eq!(file_name(""), Err(TextError::Empty));
    assert_eq!(file_name("   "), Err(TextError::Blank));
    assert_eq!(file_name("."), Err(TextError::DotName));
    assert_eq!(file_name(".."), Err(TextError::DotName));
    assert_eq!(workspace_name(".."), Ok(()));
    assert_eq!(file_name("..."), Ok(()));
    assert_eq!(file_name(&"a".repeat(MAX_FILE_NAME)), Ok(()));
    assert_eq!(
        file_name(&"a".repeat(MAX_FILE_NAME + 1)),
        Err(TextError::TooManyBytes(MAX_FILE_NAME))
    );
    assert_eq!(
        workspace_name(&"\u{e9}".repeat(65)),
        Err(TextError::TooManyBytes(MAX_FILE_NAME))
    );
    for c in [
        '\u{202E}',
        '\u{2028}',
        '\u{2029}',
        '\u{200B}',
        '\u{200D}',
        '\u{FE0F}',
        '\u{0}',
        '\u{E0041}',
        '\u{3164}',
    ] {
        let n = format!("a{c}b");
        assert_eq!(
            file_name(&n),
            Err(TextError::Hidden(c)),
            "file U+{:04X}",
            c as u32
        );
        assert_eq!(
            workspace_name(&n),
            Err(TextError::Hidden(c)),
            "workspace U+{:04X}",
            c as u32
        );
    }
}

#[test]
fn the_error_reads_as_a_phrase_after_the_thing_named() {
    assert_eq!(TextError::Empty.to_string(), "is empty");
    assert_eq!(TextError::Blank.to_string(), "is only whitespace");
    assert_eq!(TextError::DotName.to_string(), "is a dot name");
    assert_eq!(
        TextError::TooManyBytes(128).to_string(),
        "is longer than 128 bytes"
    );
    assert_eq!(
        TextError::TooManyChars(100).to_string(),
        "is longer than 100 characters"
    );
    assert_eq!(
        TextError::Hidden('\u{202E}').to_string(),
        "holds a control or hidden character (U+202E)"
    );
}

#[test]
fn an_agent_is_an_actor_that_starts_with_the_one_prefix() {
    // The prefix is defined once, here: the lanes, the layer rows and the checkpoint run rule read it from this crate.
    assert_eq!(AGENT_PREFIX, "agent:");
    assert!(is_agent("agent:claude") && is_agent("agent:"));
    for not in ["person:ana", "agent", "Agent:x", " agent:x", "xagent:x", ""] {
        assert!(!is_agent(not), "{not:?}");
    }
}
