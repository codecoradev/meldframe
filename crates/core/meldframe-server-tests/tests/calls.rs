//! E2.4: every write call is logged with who, which tool, how risky, what the gate decided and how it ended, whether
//! it was written or refused. The log holds no content: no node names, texts or commands.

use meldframe_calls::Decision;
use meldframe_host::{Op, Principal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{MAX_BATCH_OPS, Server};

struct Tools;
impl Module for Tools {
    fn name(&self) -> &'static str {
        "tools"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
        r.agent_tool("purge", "Remove a node for good.", RiskTier::Delete);
    }
}

fn who(ws: &str, actor: &str) -> Principal {
    Principal::new(ws, actor)
}

fn add(key: &str, id: &str, name: &str) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": id, "type": "frame", "name": name}))
        .unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

fn server() -> Server {
    Server::assemble_with(vec![Box::new(Tools)])
}

#[test]
fn an_allowed_write_is_logged_with_actor_tool_risk_decision_and_outcome() {
    let mut s = server();
    s.submit_tool(
        "paint",
        "f1",
        &who("ws_1", "agent:a"),
        vec![add("k1", "n1", "Hero")],
    )
    .unwrap();
    let log = s.calls(10);
    assert_eq!(log.len(), 1);
    let e = &log[0];
    assert_eq!(
        (
            e.workspace.as_str(),
            e.actor.as_str(),
            e.tool.as_str(),
            e.file.as_str()
        ),
        ("ws_1", "agent:a", "paint", "f1")
    );
    assert_eq!(
        (e.risk, e.decision, e.outcome.as_str()),
        (Some(RiskTier::Change), Some(Decision::Allow), "ok")
    );
    assert!(e.at > 0);
}

#[test]
fn a_tool_whose_policy_is_ask_is_logged_as_ask_and_nothing_is_written() {
    let mut s = server();
    let err = s.submit_tool(
        "purge",
        "f1",
        &who("ws_1", "agent:a"),
        vec![add("k1", "n1", "Hero")],
    );
    assert!(err.is_err());
    let log = s.calls(10);
    assert_eq!(log.len(), 1, "a refused call is in the log");
    assert_eq!(
        (log[0].decision, log[0].outcome.as_str(), log[0].risk),
        (Some(Decision::Ask), "forbidden", Some(RiskTier::Delete))
    );
}

#[test]
fn an_unknown_tool_is_logged_as_denied() {
    let mut s = server();
    assert!(
        s.submit_tool(
            "no_such_tool",
            "f1",
            &who("ws_1", "agent:a"),
            vec![add("k1", "n1", "Hero")]
        )
        .is_err()
    );
    let log = s.calls(10);
    assert_eq!(
        (
            log[0].tool.as_str(),
            log[0].decision,
            log[0].outcome.as_str()
        ),
        ("no_such_tool", Some(Decision::Deny), "forbidden")
    );
}

#[test]
fn a_failure_the_policy_did_not_decide_has_no_decision() {
    let mut s = server();
    let many: Vec<Op> = (0..=MAX_BATCH_OPS)
        .map(|i| add(&format!("k{i}"), &format!("n{i}"), "x"))
        .collect();
    assert!(
        s.submit_tool("paint", "f1", &who("ws_1", "agent:a"), many)
            .is_err()
    );
    let log = s.calls(10);
    assert_eq!(log[0].decision, None);
    assert_ne!(log[0].outcome, "ok");
}

#[test]
fn the_log_holds_no_content() {
    let mut s = server();
    s.submit_tool(
        "paint",
        "f1",
        &who("ws_1", "agent:a"),
        vec![add("k1", "n1", "SECRET-LAYER-NAME")],
    )
    .unwrap();
    let _ = s.submit_tool(
        "purge",
        "f1",
        &who("ws_1", "agent:a"),
        vec![add("k2", "n2", "SECRET-TWO")],
    );
    let text = format!("{:?}", s.calls(100));
    assert!(!text.contains("SECRET"), "{text}");
}

#[test]
fn calls_come_in_order_and_name_their_own_workspace() {
    let mut s = server();
    s.submit_tool(
        "paint",
        "f1",
        &who("ws_1", "agent:a"),
        vec![add("k1", "n1", "A")],
    )
    .unwrap();
    s.submit_tool(
        "paint",
        "f1",
        &who("ws_2", "agent:b"),
        vec![add("k1", "n1", "B")],
    )
    .unwrap();
    let log = s.calls(10);
    assert_eq!(
        (log[0].workspace.as_str(), log[1].workspace.as_str()),
        ("ws_1", "ws_2")
    );
    assert_eq!(s.calls(1).len(), 1, "a limit returns the newest");
    assert_eq!(s.calls(1)[0].workspace, "ws_2");
}

#[test]
fn a_code_that_is_not_a_stable_code_is_logged_as_internal() {
    use meldframe_calls::describe_code;
    assert_eq!(describe_code(None), (Some(Decision::Allow), "ok".into()));
    assert_eq!(
        describe_code(Some("forbidden")),
        (Some(Decision::Deny), "forbidden".into())
    );
    assert_eq!(describe_code(Some("invalid")), (None, "invalid".into()));
    assert_eq!(
        describe_code(Some("no_capable_client")),
        (Some(Decision::Allow), "no_capable_client".into())
    );
    for text in ["Hero title: Buy now", "", "a\nb", &"x".repeat(65)] {
        assert_eq!(describe_code(Some(text)).1, "internal", "{text:?}");
    }
}
