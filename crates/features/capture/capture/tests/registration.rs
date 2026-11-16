//! The capture module registers its tool with the Read tier, and every code is documented.

use capture::{CODES, CaptureModule};
use meldframe_module::{Policy, Registry, RiskTier};

const DOC: &str = include_str!("../../../../../docs/error-codes.md");

#[test]
fn the_tool_is_read_tier() {
    let mut reg = Registry::new();
    reg.register(&CaptureModule).unwrap();
    let t = reg.agent_tool("capture").expect("listed");
    assert_eq!((t.module, t.tier), ("capture", RiskTier::Read));
    assert_eq!(reg.policy_for("capture"), Some(Policy::Auto));
}

#[test]
fn error_codes_doc_lists_exactly_the_codes() {
    let section = DOC.split("\n## Capture codes\n").nth(1).expect("section");
    let section = section.split("\n## ").next().unwrap();
    let mut doc: Vec<&str> = section
        .lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| l[3..].split('`').next().unwrap())
        .collect();
    let mut code: Vec<&str> = CODES.to_vec();
    doc.sort();
    code.sort();
    assert_eq!(doc, code);
}
