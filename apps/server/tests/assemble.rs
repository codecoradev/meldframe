//! The composition root proves omission at build level: the tool list follows the cargo features.
//! Run `cargo test -p meldframe-server --no-default-features` for the feature-off half.

use meldframe_host::{Op, Principal, Refusal};
use meldframe_module::{Policy, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Server, SubmitError};

fn who() -> Principal {
    Principal::new("ws_dev", "agent:test")
}

fn add(key: &str) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": "n1", "type": "frame"})).unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

fn tool_names(server: &Server) -> Vec<&'static str> {
    server
        .registry()
        .agent_tools()
        .iter()
        .map(|t| t.name)
        .collect()
}

#[cfg(feature = "example")]
mod feature_on {
    use super::*;

    #[test]
    fn the_tool_list_has_example_ping_with_its_tier() {
        let server = Server::assemble();
        assert!(tool_names(&server).contains(&"example_ping"));
        let mut expected = vec!["example"];
        expected.extend(["mcp"].into_iter().filter(|_| cfg!(feature = "mcp")));
        expected.extend(
            ["capture"]
                .into_iter()
                .filter(|_| cfg!(feature = "capture")),
        );
        expected.extend(["assets"].into_iter().filter(|_| cfg!(feature = "assets")));
        let review = cfg!(feature = "design-review");
        expected.extend(["design-review"].into_iter().filter(|_| review));
        expected.extend(["export"].into_iter().filter(|_| cfg!(feature = "export")));
        let import = cfg!(feature = "variables-import");
        expected.extend(["variables-import"].into_iter().filter(|_| import));
        assert_eq!(server.registry().module_names(), expected);
        let tool = server.registry().agent_tool("example_ping").unwrap();
        assert_eq!(tool.tier, RiskTier::Read);
    }

    #[test]
    fn a_read_tool_cannot_submit_an_op_that_changes_data() {
        // The tool's tier is an upper bound on the tier of what it submits.
        let mut server = Server::assemble();
        let err = server
            .submit_tool("example_ping", "file_dev", &who(), vec![add("k1")])
            .unwrap_err();
        assert!(
            matches!(
                err,
                SubmitError::Refused(Refusal::Forbidden {
                    tier: RiskTier::Change,
                    policy: Policy::Deny
                })
            ),
            "{err:?}"
        );
    }
}

#[cfg(not(feature = "example"))]
mod feature_off {
    use super::*;

    #[test]
    fn a_build_without_the_example_feature_does_not_list_its_tool() {
        let server = Server::assemble();
        assert!(!tool_names(&server).contains(&"example_ping"));
        assert!(!server.registry().module_names().contains(&"example"));
        assert!(server.registry().agent_tool("example_ping").is_none());
    }

    #[test]
    fn a_tool_that_was_left_out_is_denied_at_the_gate() {
        let mut server = Server::assemble();
        let err = server
            .submit_tool("example_ping", "file_dev", &who(), vec![add("k1")])
            .unwrap_err();
        assert!(matches!(
            err,
            SubmitError::Refused(Refusal::Forbidden {
                policy: Policy::Deny,
                ..
            })
        ));
    }
}

#[test]
fn an_unknown_tool_is_denied() {
    let mut server = Server::assemble();
    let err = server
        .submit_tool("no_such_tool", "file_dev", &who(), vec![add("k1")])
        .unwrap_err();
    assert!(
        matches!(
            err,
            SubmitError::Refused(Refusal::Forbidden {
                policy: Policy::Deny,
                ..
            })
        ),
        "{err:?}"
    );
}

/// The capture module is compiled in only with its feature, and its tool is a Read.
#[test]
fn the_capture_tool_is_listed_only_when_the_feature_is_on() {
    let server = Server::assemble();
    let tool = server.registry().agent_tool("capture");
    if cfg!(feature = "capture") {
        let tool = tool.expect("listed");
        assert_eq!((tool.module, tool.tier), ("capture", RiskTier::Read));
    } else {
        assert!(tool.is_none());
    }
}
