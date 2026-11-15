//! Development tokens: read from configuration, never from argv; refused in release unless allowed.

use meldframe_host::Principal;
use meldframe_http::{Authenticator, DevTokens, check_dev_tokens_allowed};

#[test]
fn tokens_map_to_a_workspace_and_an_actor() {
    let t = DevTokens::parse("tok-one-1:ws_1:agent:a, tok-two-2:ws_2:human:b").unwrap();
    assert_eq!(
        t.authenticate("tok-one-1"),
        Some(Principal::new("ws_1", "agent:a"))
    );
    assert_eq!(
        t.authenticate("tok-two-2"),
        Some(Principal::new("ws_2", "human:b"))
    );
    assert_eq!(t.authenticate("tok-one"), None);
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
            !err.contains("tok:") && !err.contains("a:b:c"),
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
