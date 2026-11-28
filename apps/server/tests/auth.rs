//! Development tokens: read from configuration, never from argv; refused in release unless allowed.

use meldframe_host::Principal;
use meldframe_http::{
    Authenticator, DevTokens, check_dev_listen, check_dev_tokens_allowed, dev_tokens_warning,
};

#[test]
fn tokens_map_to_a_workspace_and_an_actor() {
    let t = DevTokens::parse("tok-one-1-abcdefgh:ws_1:agent:a, tok-two-2-abcdefgh:ws_2:human:b")
        .unwrap();
    assert_eq!(
        t.authenticate("tok-one-1-abcdefgh"),
        Some(Principal::new("ws_1", "agent:a"))
    );
    assert_eq!(
        t.authenticate("tok-two-2-abcdefgh"),
        Some(Principal::new("ws_2", "human:b"))
    );
    assert_eq!(t.authenticate("tok-one-1-abcdefg"), None);
    assert_eq!(t.authenticate(""), None);
}

#[test]
fn a_bad_spec_is_an_error_that_does_not_echo_the_secret() {
    for spec in [
        "",
        "tok",
        "tok:ws",
        ":ws:actor",
        "tok::actor",
        "tok:ws:",
        "a:b:c,a:b:d",
    ] {
        let err = DevTokens::parse(spec).unwrap_err().to_string();
        assert!(
            !err.contains("tok:") && !err.contains("a:b:c") && !err.contains("0123456789"),
            "{spec}: {err}"
        );
    }
}

/// U9: the actor of a token is held to the rule of an actor (1 to 128 bytes, no control or hidden character), because
/// it reaches the log, the lanes and every row that names a writer.
#[test]
fn the_actor_of_a_token_is_plain_text_within_128_bytes() {
    let tok = "tok-one-1-abcdefgh";
    for c in [
        '\u{202E}',
        '\u{200B}',
        '\u{2028}',
        '\u{200D}',
        '\u{034F}',
        '\n',
        '\u{E0041}',
    ] {
        let err = DevTokens::parse(&format!("{tok}:ws_1:agent:a{c}b")).unwrap_err();
        assert!(!err.to_string().contains(tok), "U+{:04X}", c as u32);
    }
    // 128 bytes are the longest, in any script: 128 ASCII characters and 64 two-byte ones fit; 65 two-byte ones do not.
    for ok in [
        "a".repeat(128),
        "\u{e9}".repeat(64),
        "agent:\u{4E2D}".to_owned(),
    ] {
        assert!(
            DevTokens::parse(&format!("{tok}:ws_1:{ok}")).is_ok(),
            "{ok}"
        );
    }
    for bad in ["a".repeat(129), "\u{e9}".repeat(65), "\u{e9}".repeat(128)] {
        assert!(DevTokens::parse(&format!("{tok}:ws_1:{bad}")).is_err());
    }
}

/// U9: one rule of a file name for every transport: 1 to 128 bytes, not `.` or `..`, not blank, no slash, no control or
/// hidden character.
#[test]
fn a_file_name_is_plain_text_within_128_bytes_without_a_slash() {
    use meldframe_http::valid_file;
    for ok in [
        "file_dev",
        "Landing page",
        "\u{4E2D}\u{6587}",
        "\u{1F468}x",
        "...",
    ] {
        assert!(valid_file(ok), "{ok}");
    }
    assert!(valid_file(&"a".repeat(128)));
    assert!(valid_file(&"\u{e9}".repeat(64)));
    for bad in ["", " ", ".", "..", "a/b", "a\\b"] {
        assert!(!valid_file(bad), "{bad:?}");
    }
    assert!(!valid_file(&"a".repeat(129)));
    assert!(!valid_file(&"\u{e9}".repeat(65)));
    for c in [
        '\u{202E}',
        '\u{2028}',
        '\u{2029}',
        '\u{200B}',
        '\u{200D}',
        '\u{FE0F}',
        '\u{0}',
        '\u{034F}',
        '\u{3164}',
        '\u{E0041}',
        '\u{600}',
    ] {
        assert!(!valid_file(&format!("a{c}b")), "U+{:04X}", c as u32);
    }
}

#[test]
fn release_builds_refuse_dev_tokens_unless_explicitly_allowed() {
    assert!(check_dev_tokens_allowed(false, None).is_ok());
    assert!(check_dev_tokens_allowed(true, None).is_err());
    assert!(check_dev_tokens_allowed(true, Some("")).is_err());
    assert!(check_dev_tokens_allowed(true, Some("0")).is_err());
    assert!(check_dev_tokens_allowed(true, Some("1")).is_ok());
}

#[test]
fn a_sixteen_character_token_is_the_shortest_accepted() {
    assert!(DevTokens::parse("0123456789abcdef:ws:actor").is_ok());
    let err = DevTokens::parse("short:ws:actor").unwrap_err().to_string();
    assert!(err.contains("16") && err.contains("bytes"), "{err}");
    // Eight two-byte characters are sixteen bytes: the length is counted in bytes.
    assert!(DevTokens::parse("éééééééé:ws:actor").is_ok());
    assert!(DevTokens::parse("ééééééé:ws:actor").is_err());
}

#[test]
fn dev_tokens_listen_on_loopback_unless_explicitly_allowed() {
    let lo = "127.0.0.1:8080".parse().unwrap();
    let lo6 = "[::1]:8080".parse().unwrap();
    let open = "0.0.0.0:8080".parse().unwrap();
    let lan = "10.1.2.3:8080".parse().unwrap();
    assert!(check_dev_listen(lo, None).is_ok() && check_dev_listen(lo6, None).is_ok());
    assert!(check_dev_listen(open, None).is_err() && check_dev_listen(lan, Some("0")).is_err());
    assert!(check_dev_listen(open, Some("1")).is_ok());
}

#[test]
fn a_release_build_started_with_the_override_is_warned_about() {
    assert!(dev_tokens_warning(true, Some("1")).is_some_and(|w| w.contains("development tokens")));
    assert!(dev_tokens_warning(true, None).is_none());
    assert!(dev_tokens_warning(false, Some("1")).is_none());
}
