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
