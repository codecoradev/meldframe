//! E2.3: the server enforces per-workspace quotas (ops per minute, storage) with stable codes, counts each
//! workspace apart, and records a refused call in the call log with its code and no content.

use meldframe_host::{Op, Principal, Refusal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Server, SubmitError};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

struct Tools;
impl Module for Tools {
    fn name(&self) -> &'static str {
        "tools"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

fn add(key: &str) -> Op {
    let node =
        serde_json::from_value(serde_json::json!({"id": key, "type": "frame", "name": "Card"}))
            .unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

fn server(ops: u64, bytes: u64) -> (Server, Arc<AtomicU64>) {
    let t = Arc::new(AtomicU64::new(0));
    let t2 = t.clone();
    let s = Server::assemble_with(vec![Box::new(Tools)])
        .with_quotas(ops, bytes, move || t2.load(Ordering::SeqCst));
    (s, t)
}

fn code(e: &SubmitError) -> String {
    e.refusal()
        .map(|r: &Refusal| r.code().to_string())
        .unwrap_or_default()
}

#[test]
fn the_ops_per_minute_of_a_workspace_are_refused_with_a_stable_code_and_reset() {
    let (mut s, t) = server(2, 1_000_000);
    let a = Principal::new("ws_a", "agent:x");
    s.submit_tool("paint", "f", &a, vec![add("k1"), add("k2")])
        .unwrap();
    let err = s
        .submit_tool("paint", "f", &a, vec![add("k3")])
        .unwrap_err();
    assert_eq!(code(&err), "quota_ops_per_minute");
    // Another workspace is not affected, and the next minute starts again.
    let b = Principal::new("ws_b", "agent:x");
    s.submit_tool("paint", "f", &b, vec![add("k1")]).unwrap();
    t.store(60_000, Ordering::SeqCst);
    s.submit_tool("paint", "f", &a, vec![add("k3")]).unwrap();
}

#[test]
fn the_storage_of_a_workspace_is_refused_with_a_stable_code() {
    let (mut s, _t) = server(1_000, 1_000);
    let a = Principal::new("ws_a", "agent:x");
    let mut last = None;
    for i in 0..50 {
        match s.submit_tool("paint", "f", &a, vec![add(&format!("k{i}"))]) {
            Ok(_) => {}
            Err(e) => {
                last = Some(e);
                break;
            }
        }
    }
    assert_eq!(
        code(&last.expect("the storage limit was never hit")),
        "quota_storage"
    );
    let b = Principal::new("ws_b", "agent:x");
    s.submit_tool("paint", "f", &b, vec![add("k0")]).unwrap();
}

#[test]
fn a_refused_quota_call_is_in_the_log_with_its_code_and_no_content() {
    let (mut s, _t) = server(1, 1_000_000);
    let a = Principal::new("ws_a", "agent:x");
    s.submit_tool("paint", "f", &a, vec![add("k1")]).unwrap();
    assert!(s.submit_tool("paint", "f", &a, vec![add("k2")]).is_err());
    let log = s.calls(10);
    assert_eq!(log.last().unwrap().outcome, "quota_ops_per_minute");
    assert!(!format!("{:?}", log.last().unwrap()).contains("Card"));
}
